use monax_sidecar::{Capabilities, SidecarError, Tool, Version, find, tessdata, version};
use std::path::Path;
use std::time::{Duration, Instant};

fn use_dir(name: &str) {
    let d = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    // SAFETY: called only from the single test fn in this binary.
    unsafe { std::env::set_var("MONAX_BIN_DIR", d) };
}

// Both scenarios mutate process env, so they share one test to avoid races.
#[test]
fn discovery_and_version() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/bin");
    // SAFETY: this is the only test in this binary, so no other thread touches env.
    unsafe { std::env::set_var("MONAX_BIN_DIR", dir) };
    let p = find(Tool::Qpdf).unwrap();
    assert!(p.starts_with(dir));
    assert_eq!(version(Tool::Qpdf).unwrap(), "qpdf version 11.9.1");
    let c = Capabilities::detect();
    let wordcraft = Some(Version("wordcraft-cli 0.12.0".into()));
    assert_eq!(c.wordcraft, wordcraft);
    assert_eq!(
        find(Tool::Wordcraft),
        Some(Path::new(dir).join("wordcraft-cli"))
    );

    use_dir("bin-fail");
    assert!(matches!(version(Tool::Qpdf), Err(SidecarError::Failed(..))));

    use_dir("bin-slow");
    let t = Instant::now();
    assert!(matches!(version(Tool::Qpdf), Err(SidecarError::Timeout(_))));
    assert!(t.elapsed() < Duration::from_secs(4));

    // Output larger than the pipe buffer must not be misreported as a timeout.
    use_dir("bin-big");
    assert_eq!(version(Tool::Qpdf).unwrap().len(), 200000);

    // tesseract's data: the bundled layout keeps it beside the binary.
    use_dir("bin-tess");
    let tess = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/bin-tess");
    assert_eq!(
        tessdata(),
        Some(std::path::Path::new(tess).join("tessdata"))
    );

    // wordcraft-cli is only ever the bundled one: never looked up on PATH.
    // SAFETY: as above.
    unsafe {
        std::env::set_var("MONAX_BIN_DIR", "/nonexistent");
        std::env::set_var("PATH", dir);
    }
    assert_eq!(find(Tool::Wordcraft), None);
    assert_eq!(Capabilities::detect().wordcraft, None);

    // SAFETY: as above.
    unsafe {
        std::env::set_var("MONAX_BIN_DIR", "/nonexistent");
        std::env::remove_var("PATH");
    }
    assert!(find(Tool::Qpdf).is_none());
    assert!(version(Tool::Qpdf).is_err());
    assert_eq!(tessdata(), None);
}
