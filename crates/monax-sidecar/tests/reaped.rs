//! A child reaped behind the runner's back (SIGCHLD ignored, so the kernel
//! reaps it): `waitid` fails with ECHILD and the runner reports it without
//! signalling the child's process group, whose id may already be reused.
use monax_sidecar::{SidecarError, Tool, run};
use std::ffi::OsString;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

#[test]
fn a_child_reaped_elsewhere_is_an_error_and_its_group_is_left_alone() {
    let d = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/bin-fork");
    // SAFETY: the only test in this binary; nothing else spawns or reads env.
    unsafe {
        std::env::set_var("MONAX_BIN_DIR", d);
        libc::signal(libc::SIGCHLD, libc::SIG_IGN);
    }
    let tmp = tempfile::tempdir().unwrap();
    let f = tmp.path().join("pid");
    let args: Vec<OsString> = vec![f.clone().into(), "exit".into()];
    let r = run(
        Tool::Qpdf,
        &args,
        Duration::from_secs(20),
        &AtomicBool::new(false),
    );
    let echild =
        matches!(&r, Err(SidecarError::Spawn("qpdf", e)) if e.raw_os_error() == Some(libc::ECHILD));
    assert!(echild, "{r:?}");
    let end = Instant::now() + Duration::from_secs(5);
    let pid: i32 = loop {
        if let Ok(p) = std::fs::read_to_string(&f)
            .unwrap_or_default()
            .trim()
            .parse()
        {
            break p;
        }
        assert!(Instant::now() < end, "no pid written");
        std::thread::sleep(Duration::from_millis(10));
    };
    // A killed grandchild is gone (reaped by launchd) well within this.
    std::thread::sleep(Duration::from_millis(500));
    // SAFETY: signal 0 only checks the process exists.
    let alive = unsafe { libc::kill(pid, 0) } == 0;
    // SAFETY: our own fixture's sleeping grandchild.
    unsafe { libc::kill(pid, libc::SIGKILL) };
    assert!(
        alive,
        "the runner signalled the group of a child it could not wait for"
    );
}
