import { useEffect, useRef, useState } from "react";
import { isTerminal, Job, jobCancel, pollJob } from "../lib/invoke";

/** Runs `start`, then follows the job it returns. */
export function useJob() {
  const [job, setJob] = useState<Job | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const stop = useRef<() => void>(() => {});
  useEffect(() => () => stop.current(), []);

  async function run(start: () => Promise<{ job_id: string }>) {
    stop.current();
    setJob(null);
    setError(null);
    setBusy(true);
    try {
      const { job_id } = await start();
      stop.current = pollJob(job_id, (j, err) => {
        if (err) setError(err);
        else if (!j) setError("Job not found");
        else setJob(j);
        if (err || !j || isTerminal(j)) setBusy(false);
      });
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
      setBusy(false);
    }
  }
  async function cancel(id: string) {
    try {
      await jobCancel(id);
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    }
  }
  return { job, error, busy, run, cancel };
}

interface Props { job: Job | null; error: string | null; onCancel: (id: string) => void }

export default function JobStatus({ job, error, onCancel }: Props) {
  const failed = job?.state.kind === "failed" ? job.state.message : error;
  return (
    <div className="job" aria-live="polite">
      {failed && <p role="alert" className="error">{failed}</p>}
      {job && !failed && (
        <>
          <p>{job.state.kind === "done" ? "Done" : job.state.kind === "cancelled" ? "Cancelled" : "Working"}</p>
          {job.state.kind === "running" || job.state.kind === "queued" ? (
            <>
              <progress max={1} value={job.progress} aria-label="Progress" />
              <button type="button" onClick={() => onCancel(job.id)}>Cancel</button>
            </>
          ) : null}
          <ul>{job.outputs.map((o) => <li key={o}>{o}</li>)}</ul>
          {job.notes?.length ? (
            <ul aria-label="Notes" className="notes">{job.notes.map((n) => <li key={n}>{n}</li>)}</ul>
          ) : null}
        </>
      )}
    </div>
  );
}
