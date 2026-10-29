//! The one place sidecar processes are spawned.
use crate::{SidecarError, Tool, find};
use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Exit status and captured output (decoded lossily as UTF-8) of a finished run.
#[derive(Debug)]
pub struct RunOutput {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

/// Captured stdout cap. qpdf `--json` of a large real file (outlines, pages)
/// stays well below it; more is refused, never parsed partially.
const STDOUT_LIMIT: u64 = 16 << 20;
/// Captured stderr cap: callers use the last line (qpdf prints its fatal error
/// last), so a flood of warnings from a crafted file keeps its last 64 KiB.
const STDERR_LIMIT: usize = 64 << 10;

/// Captured bytes (at most `limit`) and whether more were written.
type Capture = (Vec<u8>, bool);

/// Reads at most `limit` bytes of `pipe`, then discards the rest so the child
/// never blocks on a full pipe.
fn drain(pipe: Option<impl Read + Send + 'static>, limit: u64) -> Option<JoinHandle<Capture>> {
    pipe.map(|mut p| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = p.by_ref().take(limit).read_to_end(&mut buf);
            let more = std::io::copy(&mut p, &mut std::io::sink()).is_ok_and(|n| n > 0);
            (buf, more)
        })
    })
}

/// Keeps the last `limit` bytes of `pipe`, reading in chunks so the child never
/// blocks; memory stays under twice `limit`.
fn drain_tail(
    pipe: Option<impl Read + Send + 'static>,
    limit: usize,
) -> Option<JoinHandle<Capture>> {
    pipe.map(|mut p| {
        std::thread::spawn(move || {
            let (mut buf, mut chunk) = (Vec::new(), [0u8; 8192]);
            loop {
                let n = match p.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                };
                buf.extend_from_slice(&chunk[..n]);
                if buf.len() > 2 * limit {
                    buf.drain(..buf.len() - limit);
                }
            }
            let more = buf.len() > limit;
            buf.drain(..buf.len().saturating_sub(limit));
            (buf, more)
        })
    })
}

/// Joins a reader. It ends: the group kill closed every pipe end it held.
// ponytail: a grandchild that left the group (setsid, or setpgid into a group
// of its own) keeps its pipe ends and blocks this; none of qpdf, tesseract or
// wordcraft-cli does. Upgrade path: bound the join or close our read ends on exit.
fn collect(h: Option<JoinHandle<Capture>>) -> Capture {
    h.and_then(|h| h.join().ok()).unwrap_or_default()
}

/// True once `child` has exited, without reaping it: until it is reaped its
/// pid, which is also its group's id, cannot be reused, so [`kill_group`]
/// never signals an unrelated group.
fn exited(child: &Child) -> std::io::Result<bool> {
    // SAFETY: siginfo_t is plain data; waitid only writes into it.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let flags = libc::WEXITED | libc::WNOHANG | libc::WNOWAIT;
    // SAFETY: valid pid of our own unreaped child and a valid out pointer.
    let r = unsafe { libc::waitid(libc::P_PID, child.id(), &mut info, flags) };
    if r == -1 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: waitid filled `info` (or left it zeroed: still running).
    Ok(unsafe { info.si_pid() } != 0)
}

/// SIGKILL to the child's whole process group (it leads it: spawned with
/// `process_group(0)`), then reaps the child. Grandchildren are killed
/// with it; launchd reaps them.
fn kill_group(child: &mut Child) -> std::io::Result<ExitStatus> {
    // SAFETY: plain syscall; the group id is our unreaped child's pid.
    unsafe { libc::killpg(child.id() as libc::pid_t, libc::SIGKILL) };
    child.wait()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Runs `tool` with `args` as an argv array (never through a shell), stdin
/// closed, stdout and stderr drained concurrently (stdout capped at 16 MiB,
/// stderr keeps its last 64 KiB, the rest discarded). Polls every 10 ms; when
/// `timeout` elapses or `cancel` becomes true the child is killed and reaped.
/// The child leads a process group of its own; the whole group is killed on
/// timeout, on cancel and once the child exits, so no grandchild outlives
/// the run (R33). A non-zero exit is not an error: callers interpret `status`.
///
/// # Errors
/// `NotFound` (tool absent), `Spawn` (cannot start or wait), `Timeout`,
/// `Cancelled`, `OutputTooLarge` (stdout over its cap).
pub fn run(
    tool: Tool,
    args: &[OsString],
    timeout: Duration,
    cancel: &AtomicBool,
) -> Result<RunOutput, SidecarError> {
    run_env(tool, args, &[], timeout, cancel)
}

/// [`run`] with `env` added to the child's environment only (never this
/// process's).
///
/// # Errors
/// As [`run`].
pub fn run_env(
    tool: Tool,
    args: &[OsString],
    env: &[(&str, &OsStr)],
    timeout: Duration,
    cancel: &AtomicBool,
) -> Result<RunOutput, SidecarError> {
    let name = tool.bin();
    let path = find(tool).ok_or(SidecarError::NotFound(name))?;
    let mut cmd = Command::new(path);
    for var in tool.env_removed() {
        cmd.env_remove(var);
    }
    let mut child = cmd
        .args(args)
        .envs(env.iter().copied())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()
        .map_err(|e| SidecarError::Spawn(name, e))?;
    let out = drain(child.stdout.take(), STDOUT_LIMIT);
    let err = drain_tail(child.stderr.take(), STDERR_LIMIT);
    let start = Instant::now();
    let fail = loop {
        match exited(&child) {
            Ok(true) => {
                let status = kill_group(&mut child).map_err(|e| SidecarError::Spawn(name, e))?;
                let (stdout, too_big) = collect(out);
                if too_big {
                    return Err(SidecarError::OutputTooLarge(name, STDOUT_LIMIT));
                }
                return Ok(RunOutput {
                    status,
                    stdout: text(&stdout),
                    stderr: text(&collect(err).0),
                });
            }
            Ok(false) if cancel.load(Ordering::Relaxed) => break SidecarError::Cancelled(name),
            Ok(false) if start.elapsed() >= timeout => break SidecarError::Timeout(name),
            Ok(false) => std::thread::sleep(Duration::from_millis(10)),
            // ECHILD: reaped elsewhere (SIGCHLD ignored), so its pid and
            // group id may be reused: signal nothing.
            Err(e) => return Err(SidecarError::Spawn(name, e)),
        }
    };
    let _ = kill_group(&mut child);
    // Reader threads are detached: they end when the pipes close.
    Err(fail)
}
