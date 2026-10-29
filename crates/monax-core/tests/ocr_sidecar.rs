//! OCR with tesseract missing, failing or cancelled: the exact reason, no
//! output, the page image private and removed, TESSDATA_PREFIX set on the
//! child only (from the bundled layout).
mod common;
use common::copy_fixture;
use common::ocr::job;
use monax_core::ocr;
use monax_core::{JobState, Registry};
use std::time::{Duration, Instant};

const TESS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../monax-sidecar/tests/fixtures/bin-tess"
);

// Every step mutates process env, so this binary has one test.
#[test]
fn missing_failing_and_cancelled_tesseract_leave_no_output() {
    require_qpdf!();
    let (dir, scan) = copy_fixture("invoice-scan.pdf", "scan.pdf");
    let out = dir.path().join("o.pdf");
    let eng = vec!["eng".to_string()];
    let path = std::env::var_os("PATH").unwrap();
    // SAFETY: the only test in this binary.
    unsafe { std::env::set_var("PATH", "/usr/bin:/bin") };
    let (i, o, l) = (scan.clone(), out.clone(), eng.clone());
    let (r, _) = job(move |s| ocr::run(&i, &o, &l, 72, s));
    assert_eq!(r.unwrap_err().to_string(), "tesseract not found");
    assert!(ocr::languages().is_empty());

    // The bundled layout: tesseract with tessdata beside it; qpdf on PATH.
    let record = dir.path().join("record.txt");
    // SAFETY: as above.
    unsafe {
        std::env::set_var("PATH", &path);
        std::env::set_var("MONAX_BIN_DIR", TESS);
        std::env::set_var("MONAX_TESS_RECORD", &record);
    }
    assert_eq!(ocr::languages(), ["eng"]);
    let (i, o, l) = (scan.clone(), out.clone(), eng.clone());
    let (r, _) = job(move |s| ocr::run(&i, &o, &l, 72, s));
    let data = format!("{TESS}/tessdata");
    assert_eq!(
        r.unwrap_err().to_string(),
        format!("tesseract failed: Error opening data file {data}/eng.traineddata")
    );
    let log = std::fs::read_to_string(&record).unwrap();
    assert!(log.contains(&format!("prefix {data}\n")), "{log}");
    for want in [
        "arg -l\narg eng\narg --dpi\narg 72\n",
        "arg tessedit_create_pdf=1\n",
    ] {
        assert!(log.contains(want), "{log}");
    }
    let (mode, png) = log
        .lines()
        .find_map(|l| l.strip_prefix("mode ")?.split_once(' '))
        .unwrap();
    assert_eq!(mode, "600");
    assert!(
        !std::path::Path::new(png).exists(),
        "page image left: {png}"
    );
    assert!(std::env::var_os("TESSDATA_PREFIX").is_none());
    std::fs::remove_file(&record).unwrap();

    // Cancel kills a running tesseract; the job ends cancelled, no output.
    // SAFETY: as above.
    unsafe { std::env::set_var("MONAX_TESS_MODE", "slow") };
    let reg = Registry::new();
    let (i, o) = (scan.clone(), out.clone());
    let id = reg.submit(move |s| ocr::run(&i, &o, &eng, 72, &s).map(|p| vec![p]));
    let end = Instant::now() + Duration::from_secs(20);
    while !record.exists() {
        assert!(Instant::now() < end, "tesseract never started");
        std::thread::sleep(Duration::from_millis(10));
    }
    let t = Instant::now();
    reg.cancel(id);
    let j = reg.wait(id).unwrap();
    assert_eq!(j.state, JobState::Cancelled);
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
    assert!(j.outputs.is_empty());
    std::fs::remove_file(&record).unwrap();
    assert_eq!(common::names(dir.path()), ["scan.pdf"]);
}
