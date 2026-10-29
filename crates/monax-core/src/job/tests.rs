use super::*;

fn expired(reg: &Registry, id: JobId) {
    let job = Job {
        id,
        state: JobState::Done,
        progress: 1.0,
        outputs: vec![],
        notes: vec![],
    };
    let finished = Instant::now().checked_sub(Duration::from_secs(1));
    reg.0.lock().insert(
        id,
        Entry {
            job,
            cancel: Arc::new(AtomicBool::new(false)),
            finished,
        },
    );
}

#[test]
fn wait_returns_expired_target_instead_of_pruning_it() {
    let reg = Registry::with_ttl(Duration::ZERO);
    expired(&reg, JobId(1));
    assert_eq!(reg.wait(JobId(1)).map(|j| j.state), Some(JobState::Done));
}

#[test]
fn status_prunes_expired_jobs() {
    let reg = Registry::with_ttl(Duration::ZERO);
    expired(&reg, JobId(1));
    assert!(reg.status(JobId(1)).is_none());
}

#[test]
fn submit_prunes_other_expired_jobs() {
    let reg = Registry::with_ttl(Duration::ZERO);
    expired(&reg, JobId(100));
    reg.submit(|_| Ok::<_, String>(vec![]));
    assert!(!reg.0.lock().contains_key(&JobId(100)));
}

#[test]
fn clones_share_jobs() {
    let a = Registry::new();
    let b = a.clone();
    let id = a.submit(|_| Ok::<_, String>(vec![]));
    assert_eq!(b.wait(id).map(|j| j.state), Some(JobState::Done));
}
