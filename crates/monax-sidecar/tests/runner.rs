use monax_sidecar::{SidecarError, Tool, run, run_env};
use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const LONG: Duration = Duration::from_secs(10);

fn use_dir(name: &str) {
    let d = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    // SAFETY: called only from the single test fn in this binary.
    unsafe { std::env::set_var("MONAX_BIN_DIR", d) };
}

// Every scenario mutates process env, so they share one test to avoid races.
#[test]
fn run_captures_times_out_cancels() {
    let never = AtomicBool::new(false);

    // argv is passed verbatim (no shell): "a b" stays one argument.
    use_dir("bin-args");
    let args: Vec<OsString> = vec!["a b".into(), "c".into()];
    let out = run(Tool::Qpdf, &args, LONG, &never).unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(out.stdout, "out:a b\nout:c\n");
    assert_eq!(out.stderr, "err:2\n");

    // Extra environment reaches the child only, never this process.
    use_dir("bin-env");
    let env = [("MONAX_TEST_ENV", std::ffi::OsStr::new("x y"))];
    let out = run_env(Tool::Qpdf, &[], &env, LONG, &never).unwrap();
    assert_eq!(out.stdout, "x y");
    assert!(std::env::var_os("MONAX_TEST_ENV").is_none());
    // WordCraft never sees a WORDCRAFT_PASSWORD of this process.
    // SAFETY: as in use_dir.
    unsafe { std::env::set_var("WORDCRAFT_PASSWORD", "secret") };
    let out = run(Tool::Wordcraft, &[], LONG, &never).unwrap();
    assert_eq!(out.stdout, "unset");
    // SAFETY: as in use_dir.
    unsafe { std::env::remove_var("WORDCRAFT_PASSWORD") };

    // Non-zero exit is reported, not turned into an error.
    use_dir("bin-fail");
    let out = run(Tool::Qpdf, &[], LONG, &never).unwrap();
    assert_eq!(out.status.code(), Some(1));

    // Output larger than a pipe buffer must not deadlock.
    use_dir("bin-big");
    let out = run(Tool::Qpdf, &[], LONG, &never).unwrap();
    assert_eq!(out.stdout.trim_end().len(), 200000);

    // Captures are capped: stderr keeps its last 64 KiB so the fatal last
    // line survives a warning flood, the child still runs to completion;
    // stdout over 16 MiB is an error, never a partial parse.
    use_dir("bin-huge");
    let out = run(Tool::Qpdf, &["err".into()], LONG, &never).unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(out.stderr.len(), 64 * 1024);
    assert_eq!(
        out.stderr.lines().last(),
        Some("qpdf: fatal: the real reason")
    );
    let r = run(Tool::Qpdf, &[], LONG, &never);
    assert!(
        matches!(r, Err(SidecarError::OutputTooLarge("qpdf", 16_777_216))),
        "{r:?}"
    );

    // Timeout kills the child (fixture sleeps 5s).
    use_dir("bin-slow");
    let t = Instant::now();
    let r = run(Tool::Qpdf, &[], Duration::from_millis(200), &never);
    assert!(matches!(r, Err(SidecarError::Timeout("qpdf"))), "{r:?}");
    assert!(t.elapsed() < Duration::from_secs(2));

    // Cancel flag set from another thread kills the child.
    let cancel = AtomicBool::new(false);
    let t = Instant::now();
    let r = std::thread::scope(|s| {
        s.spawn(|| {
            std::thread::sleep(Duration::from_millis(100));
            cancel.store(true, Ordering::Relaxed);
        });
        run(Tool::Qpdf, &[], LONG, &cancel)
    });
    assert!(matches!(r, Err(SidecarError::Cancelled("qpdf"))), "{r:?}");
    assert!(t.elapsed() < Duration::from_secs(2));

    // Missing tool is a typed error naming it.
    // SAFETY: as above.
    unsafe {
        std::env::set_var("MONAX_BIN_DIR", "/nonexistent");
        std::env::remove_var("PATH");
    }
    let r = run(Tool::Qpdf, &[], LONG, &never);
    assert!(matches!(r, Err(SidecarError::NotFound("qpdf"))), "{r:?}");
}
