import { useEffect, useRef, useState } from "react";
import JobStatus, { useJob } from "../components/JobStatus";
import PasswordPrompt, { passwordNeed } from "../components/PasswordPrompt";
import { callTool, choices } from "../lib/invoke";
import { buildInput, FieldSpec, humanize, ToolEntry, Values } from "../tools";

/** `blocked`: why the tool cannot run here (no WordCraft); Apply stays off. */
export default function ToolPanel({ tool, files, blocked }: { tool: ToolEntry; files: string[]; blocked?: string }) {
  const [values, setValues] = useState<Values>({});
  const { job, error, busy, run, cancel } = useJob();
  const ready = files.length >= (tool.fileKeys?.length ?? 1) && !busy && !blocked;
  const set = (name: string, v: string | number | boolean) => setValues((p) => ({ ...p, [name]: v }));

  const failure = job?.state.kind === "failed" ? job.state.message : error;
  const need = tool.ownPassword || busy ? null : passwordNeed(failure);

  // Values of the last Apply: a prompt retry resends them with the input passwords.
  const pending = useRef<Values>({});
  /** Runs the tool; `passwords` (from the prompt) open the files and retry the last Apply. */
  function apply(passwords: string[] = []) {
    const snapshot = passwords.length ? pending.current : values;
    pending.current = snapshot;
    void run(async () => callTool(tool.id, buildInput(tool, files, snapshot, passwords)));
  }
  // Typed secrets are cleared once the job ends, unless it ended asking for an input password.
  const settled = !busy && (failure != null || job?.state.kind === "done" || job?.state.kind === "cancelled");
  useEffect(() => {
    if (!settled || need) return;
    const secret = (k: string) => tool.options.some((f) => f.name === k && f.kind === "password");
    setValues((v) => Object.fromEntries(Object.entries(v).filter(([k]) => !secret(k))));
    pending.current = {};
  }, [settled, need, tool]);
  return (
    <form onSubmit={(e) => { e.preventDefault(); apply(); }}>
      <h2>{tool.title}</h2>
      {tool.hint && <p>{tool.hint}</p>}
      {blocked && <p className="error">{blocked}</p>}
      <p className="muted">{files.length ? files.join(", ") : dropHint(tool)}</p>
      {tool.options.map((f) => (
        <label key={f.name} className="field">
          <span>{f.label}</span>
          <Field f={f} value={values[f.name] ?? f.default} set={(v) => set(f.name, v)} />
        </label>
      ))}
      <button type="submit" disabled={!ready}>Apply</button>
      {need && <PasswordPrompt files={files} wrong={need === "wrong"} onSubmit={apply} />}
      <JobStatus job={job} error={error} onCancel={(id) => void cancel(id)} />
    </form>
  );
}

function dropHint(tool: ToolEntry): string {
  if (tool.accepts) return `Drop ${tool.accepts.join(", ")} files onto the window.`;
  return tool.fileKeys ? `Drop ${tool.fileKeys.length} PDFs onto the window.` : "Drop a PDF onto the window.";
}

type FieldProps = { f: FieldSpec; value: string | number | boolean | undefined; set: (v: string | boolean) => void };

function Field({ f, value, set }: FieldProps) {
  if (f.kind === "checkbox") return <input type="checkbox" checked={Boolean(value)} onChange={(e) => set(e.target.checked)} />;
  if (f.kind === "select") return <Select f={f} value={value} set={set} />;
  const type = f.kind === "number" || f.kind === "color" || f.kind === "password" ? f.kind : "text";
  const placeholder = f.kind === "pages" || f.kind === "ranges" ? (f.optional ? "All pages" : "1-3, 5") : undefined;
  const shown = value ?? (f.kind === "color" ? "#000000" : "");
  const step = f.kind === "number" ? "any" : undefined; // default step 1 rejects 0.3
  const auto = f.kind === "password" ? "off" : undefined; // never offer to save document passwords
  return <input type={type} step={step} placeholder={placeholder} autoComplete={auto} value={String(shown)} onChange={(e) => set(e.target.value)} />;
}

/** A select of `f.choices`, or of what `f.choicesCommand` returns (none while loading or on failure);
 * a blank "Default" (the core default) only when the field has no default of its own. */
function Select({ f, value, set }: FieldProps) {
  const [loaded, setLoaded] = useState<string[]>([]);
  useEffect(() => {
    if (!f.choicesCommand) return;
    let live = true;
    choices(f.choicesCommand).then((c) => live && setLoaded(c), () => {});
    return () => { live = false; };
  }, [f.choicesCommand]);
  return (
    <select value={String(value ?? "")} onChange={(e) => set(e.target.value)}>
      {f.default === undefined && <option value="">Default</option>}
      {(f.choices ?? loaded).map((c) => <option key={c} value={c}>{humanize(c)}</option>)}
    </select>
  );
}
