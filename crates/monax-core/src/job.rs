//! Job registry: one thread per job, cooperative cancel, lazy 1h GC.
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

mod model;
pub use model::{Job, JobId, JobState};

struct Entry {
    job: Job,
    cancel: Arc<AtomicBool>,
    finished: Option<Instant>,
}

struct Inner {
    jobs: Mutex<HashMap<JobId, Entry>>,
    changed: Condvar,
    next: AtomicU64,
    ttl: Duration,
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, HashMap<JobId, Entry>> {
        self.jobs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn prune(&self, jobs: &mut HashMap<JobId, Entry>, keep: Option<JobId>) {
        let ttl = self.ttl;
        jobs.retain(|id, e| Some(*id) == keep || e.finished.is_none_or(|t| t.elapsed() <= ttl));
    }

    fn update(&self, id: JobId, f: impl FnOnce(&mut Entry)) {
        if let Some(e) = self.lock().get_mut(&id) {
            f(e);
        }
        self.changed.notify_all();
    }
}

/// Handed to a job closure to report progress and observe cancellation.
#[derive(Clone)]
pub struct ProgressSink {
    id: JobId,
    inner: Arc<Inner>,
    cancel: Arc<AtomicBool>,
}

impl ProgressSink {
    /// Sets progress, clamped to 0.0..=1.0.
    pub fn set(&self, p: f32) {
        let p = if p.is_nan() { 0.0 } else { p.clamp(0.0, 1.0) };
        self.inner.update(self.id, |e| e.job.progress = p);
    }

    /// Appends a note for the user to the job (see [`Job::notes`]).
    pub fn note(&self, text: impl Into<String>) {
        let text = text.into();
        self.inner.update(self.id, |e| e.job.notes.push(text));
    }

    /// True once `Registry::cancel` was called; long loops should poll this.
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// The flag behind [`Self::is_cancelled`], for a sidecar run to kill on.
    pub(crate) fn cancel_flag(&self) -> &AtomicBool {
        &self.cancel
    }
}

/// Runs jobs on worker threads and tracks their state. Clones share the same
/// jobs, so the GUI and the MCP server see one registry.
#[derive(Clone)]
pub struct Registry(Arc<Inner>);

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    /// New registry; finished jobs are pruned 1h after completion.
    pub fn new() -> Self {
        Self::with_ttl(Duration::from_secs(3600))
    }

    /// Like `new` with a custom retention for finished jobs.
    pub(crate) fn with_ttl(ttl: Duration) -> Self {
        Self(Arc::new(Inner {
            jobs: Mutex::new(HashMap::new()),
            changed: Condvar::new(),
            next: AtomicU64::new(1),
            ttl,
        }))
    }

    /// Runs `f` on its own thread. An `Err` becomes `Failed(msg)`, a panic becomes
    /// `Failed`, and a job cancelled meanwhile ends `Cancelled` with no outputs; the
    /// files its closure returned are removed (best effort).
    pub fn submit<F, E>(&self, f: F) -> JobId
    where
        F: FnOnce(ProgressSink) -> Result<Vec<PathBuf>, E> + Send + 'static,
        E: std::fmt::Display,
    {
        let inner = &self.0;
        let id = JobId(inner.next.fetch_add(1, Ordering::Relaxed));
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut jobs = inner.lock();
            inner.prune(&mut jobs, None);
            let job = Job {
                id,
                state: JobState::Queued,
                progress: 0.0,
                outputs: vec![],
                notes: vec![],
            };
            jobs.insert(
                id,
                Entry {
                    job,
                    cancel: cancel.clone(),
                    finished: None,
                },
            );
        }
        let sink = ProgressSink {
            id,
            inner: inner.clone(),
            cancel: cancel.clone(),
        };
        let inner = inner.clone();
        std::thread::spawn(move || {
            inner.update(id, |e| e.job.state = JobState::Running);
            let res = catch_unwind(AssertUnwindSafe(|| f(sink)));
            let cancelled = cancel.load(Ordering::Relaxed);
            if cancelled && let Ok(Ok(outputs)) = &res {
                crate::output::remove_all(outputs);
            }
            inner.update(id, |e| {
                e.finished = Some(Instant::now());
                if cancelled {
                    e.job.state = JobState::Cancelled;
                    e.job.outputs.clear();
                    return;
                }
                match res {
                    Ok(Ok(outputs)) => {
                        e.job.state = JobState::Done;
                        e.job.progress = 1.0;
                        e.job.outputs = outputs;
                    }
                    Ok(Err(err)) => e.job.state = JobState::Failed(err.to_string()),
                    Err(_) => e.job.state = JobState::Failed("job panicked".into()),
                }
            });
        });
        id
    }

    /// Current snapshot, or `None` for an unknown or pruned id.
    pub fn status(&self, id: JobId) -> Option<Job> {
        let mut jobs = self.0.lock();
        self.0.prune(&mut jobs, None);
        jobs.get(&id).map(|e| e.job.clone())
    }

    /// Blocks until the job is terminal; `None` for an unknown id.
    pub fn wait(&self, id: JobId) -> Option<Job> {
        let mut jobs = self.0.lock();
        self.0.prune(&mut jobs, Some(id));
        loop {
            let e = jobs.get(&id)?;
            if e.finished.is_some() {
                return Some(e.job.clone());
            }
            jobs = self
                .0
                .changed
                .wait(jobs)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Requests cancellation. No-op for unknown or already finished jobs.
    pub fn cancel(&self, id: JobId) {
        if let Some(e) = self.0.lock().get(&id).filter(|e| e.finished.is_none()) {
            e.cancel.store(true, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests;
