use monax_core::{JobState, Registry};
use std::sync::mpsc;

#[test]
fn cancelled_job_outputs_are_removed() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("out.pdf");
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let reg = Registry::new();
    let f = file.clone();
    let id = reg.submit(move |_| {
        std::fs::write(&f, b"x").map_err(|e| e.to_string())?;
        started_tx.send(()).map_err(|e| e.to_string())?;
        release_rx.recv().map_err(|e| e.to_string())?;
        Ok::<_, String>(vec![f])
    });
    started_rx.recv().unwrap();
    reg.cancel(id);
    release_tx.send(()).unwrap();
    let job = reg.wait(id).unwrap();
    assert_eq!(job.state, JobState::Cancelled);
    assert!(job.outputs.is_empty());
    assert!(!file.exists());
}

#[test]
fn finished_job_keeps_its_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("out.pdf");
    let f = file.clone();
    let reg = Registry::new();
    let id = reg.submit(move |_| {
        std::fs::write(&f, b"x").map_err(|e| e.to_string())?;
        Ok::<_, String>(vec![f])
    });
    assert_eq!(reg.wait(id).unwrap().state, JobState::Done);
    assert!(file.exists());
}

#[test]
fn job_serializes_for_the_frontend() {
    let reg = Registry::new();
    let id = reg.submit(|_| Err::<Vec<std::path::PathBuf>, _>("boom"));
    let job = reg.wait(id).unwrap();
    let v = serde_json::to_value(&job).unwrap();
    assert!(v["id"].is_string());
    assert_eq!(v["state"]["kind"], "failed");
    assert_eq!(v["state"]["message"], "boom");
    assert_eq!(v["progress"], 0.0);
    let back: monax_core::JobId = serde_json::from_value(v["id"].clone()).unwrap();
    assert_eq!(back, id);
}
