//! Office conversion helpers: the fake `wordcraft-cli`
//! (tests/fixtures/bin-wordcraft) and what it logged.
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Puts the fake wordcraft-cli first in discovery, logging to `log`,
/// behaving as `mode` (ok, none, fail, hang). Only for a test binary's
/// single test.
pub fn fake(mode: &str, log: &Path) {
    // SAFETY: each office test binary has one test fn: no other thread reads env.
    unsafe {
        std::env::set_var("MONAX_BIN_DIR", super::fixture("bin-wordcraft"));
        std::env::set_var("FAKE_WORDCRAFT", mode);
        std::env::set_var("FAKE_WORDCRAFT_LOG", log);
    }
}

/// The argv the fake got, one entry per element.
pub fn argv(log: &Path) -> Vec<String> {
    let s = std::fs::read_to_string(log).unwrap();
    s.lines().map(String::from).collect()
}

/// The private temp dir the run wrote into (the output's parent), to check
/// it is gone after the run.
pub fn temp(log: &Path) -> PathBuf {
    let out = PathBuf::from(argv(log).pop().unwrap());
    out.parent().unwrap().to_path_buf()
}

/// The real `wordcraft-cli`: `MONAX_BIN_DIR` when it holds one, else the
/// workspace's `sidecars/` (scripts/fetch-sidecars.sh), made the discovery
/// dir. `None` (with a skip notice) when neither has it. Only for a test
/// binary's single test.
pub fn real() -> Option<PathBuf> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sidecars");
    let dirs = [std::env::var_os("MONAX_BIN_DIR").map(PathBuf::from)];
    let dir = (dirs.into_iter().flatten())
        .chain([PathBuf::from(root)])
        .find(|d| d.join("wordcraft-cli").is_file());
    let Some(dir) = dir else {
        let msg = "SKIPPED: wordcraft-cli not built (scripts/fetch-sidecars.sh)\n";
        let _ = std::io::Write::write_all(&mut std::io::stderr(), msg.as_bytes());
        return None;
    };
    // SAFETY: only from a test binary's single test.
    unsafe { std::env::set_var("MONAX_BIN_DIR", &dir) };
    Some(dir)
}

/// The pid the hanging fake wrote, waiting for it.
pub fn pid(log: &Path) -> i32 {
    let f = log.with_extension("log.pid");
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(p) = std::fs::read_to_string(&f)
            .unwrap_or_default()
            .trim()
            .parse()
        {
            return p;
        }
        assert!(Instant::now() < end, "fake wordcraft-cli wrote no pid");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// True once process `pid` is gone (polls up to 3 s).
pub fn gone(pid: i32) -> bool {
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
