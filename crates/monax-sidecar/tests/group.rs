//! R33: a sidecar runs in its own process group and the whole group is
//! killed on timeout, on cancel and once the sidecar exits, so no
//! grandchild outlives the run or holds its pipes open.
use monax_sidecar::{SidecarError, Tool, run};
use std::ffi::OsString;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The grandchild's pid, written by the fixture.
fn pid(file: &Path) -> i32 {
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        let s = std::fs::read_to_string(file).unwrap_or_default();
        if let Ok(p) = s.trim().parse() {
            return p;
        }
        assert!(Instant::now() < end, "no pid written");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// True once `pid` no longer exists (killed and reaped by launchd).
fn gone(pid: i32) -> bool {
    let end = Instant::now() + Duration::from_secs(3);
    while Instant::now() < end {
        // SAFETY: signal 0 only checks that the process exists.
        if unsafe { libc::kill(pid, 0) } == -1 {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn args(file: &Path, mode: &str) -> Vec<OsString> {
    vec![file.into(), mode.into()]
}

// Every scenario mutates process env, so they share one test to avoid races.
#[test]
fn the_whole_process_group_goes() {
    let d = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/bin-fork");
    // SAFETY: the only test in this binary.
    unsafe { std::env::set_var("MONAX_BIN_DIR", d) };
    let tmp = tempfile::tempdir().unwrap();
    let never = AtomicBool::new(false);
    let long = Duration::from_secs(20);
    // wordcraft-cli (a link to the same fixture) runs the same way (W2).
    for tool in [Tool::Qpdf, Tool::Wordcraft] {
        // The leader exits while a grandchild holds stdout: the run ends at once.
        let f = tmp.path().join(format!("{}-exit", tool.bin()));
        let t = Instant::now();
        let out = run(tool, &args(&f, "exit"), long, &never).unwrap();
        assert_eq!(
            (out.status.code(), out.stdout.as_str()),
            (Some(0), "done\n")
        );
        assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
        assert!(gone(pid(&f)), "grandchild outlived a finished run");

        // Timeout kills the grandchild too.
        let f = tmp.path().join(format!("{}-timeout", tool.bin()));
        let r = run(tool, &args(&f, "wait"), Duration::from_millis(300), &never);
        let n = tool.bin();
        assert!(
            matches!(r, Err(SidecarError::Timeout(m)) if m == n),
            "{r:?}"
        );
        assert!(gone(pid(&f)), "grandchild outlived a timeout");

        // Cancel kills the grandchild too.
        let f = tmp.path().join(format!("{}-cancel", tool.bin()));
        let cancel = AtomicBool::new(false);
        let r = std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(Duration::from_millis(300));
                cancel.store(true, Ordering::Relaxed);
            });
            run(tool, &args(&f, "wait"), long, &cancel)
        });
        let n = tool.bin();
        assert!(
            matches!(r, Err(SidecarError::Cancelled(m)) if m == n),
            "{r:?}"
        );
        assert!(gone(pid(&f)), "grandchild outlived a cancel");
    }
}
