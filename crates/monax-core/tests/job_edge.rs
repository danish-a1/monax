use monax_core::{JobId, JobState, Registry};
use std::time::Duration;

#[test]
fn closure_error_marks_failed() {
    let reg = Registry::new();
    let id = reg.submit(|_| Err::<Vec<_>, _>("boom"));
    assert_eq!(reg.wait(id).unwrap().state, JobState::Failed("boom".into()));
}

#[test]
fn panic_marks_failed() {
    let reg = Registry::new();
    let id = reg.submit(|_| -> Result<Vec<_>, String> { panic!("kaput") });
    assert!(matches!(reg.wait(id).unwrap().state, JobState::Failed(_)));
    // registry still usable after a panic
    let ok = reg.submit(|_| Ok::<_, String>(vec![]));
    assert_eq!(reg.wait(ok).unwrap().state, JobState::Done);
}

#[test]
fn cancel_running_job_ends_cancelled_with_no_outputs() {
    let reg = Registry::new();
    let (tx, rx) = std::sync::mpsc::channel();
    let id = reg.submit(move |p| {
        tx.send(()).unwrap();
        while !p.is_cancelled() {
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok::<_, String>(vec!["/tmp/partial.pdf".into()])
    });
    rx.recv().unwrap();
    reg.cancel(id);
    let job = reg.wait(id).unwrap();
    assert_eq!(job.state, JobState::Cancelled);
    assert!(job.outputs.is_empty());
}

#[test]
fn cancel_after_done_is_noop() {
    let reg = Registry::new();
    let id = reg.submit(|_| Ok::<_, String>(vec![]));
    reg.wait(id).unwrap();
    reg.cancel(id);
    assert_eq!(reg.status(id).unwrap().state, JobState::Done);
}

#[test]
fn progress_is_clamped() {
    let reg = Registry::new();
    let (tx, rx) = std::sync::mpsc::channel();
    let (go, wait_go) = std::sync::mpsc::channel::<()>();
    let id = reg.submit(move |p| {
        p.set(5.0);
        tx.send(()).unwrap();
        wait_go.recv().unwrap();
        Ok::<_, String>(vec![])
    });
    rx.recv().unwrap();
    assert_eq!(reg.status(id).unwrap().progress, 1.0);
    go.send(()).unwrap();
    reg.wait(id).unwrap();
}

#[test]
fn unknown_id_is_none() {
    let reg = Registry::new();
    assert!(reg.status(JobId(999)).is_none());
    assert!(reg.wait(JobId(999)).is_none());
    reg.cancel(JobId(999));
}
