import { useEffect, useState } from "react";
import JobStatus, { useJob } from "../components/JobStatus";
import PasswordPrompt, { passwordNeed } from "../components/PasswordPrompt";
import { callTool, FormField, listFields } from "../lib/invoke";

/** Fills a PDF form: one control per field; Apply sends only the values changed. */
export default function FillForm({ path }: { path: string }) {
  const [fields, setFields] = useState<FormField[] | null>(null);
  const [values, setValues] = useState<Record<string, string>>({});
  const [loadError, setLoadError] = useState<string | null>(null);
  /** Opens an encrypted file; asked for when it needs one (App keys this panel by path). */
  const [password, setPassword] = useState<string | undefined>(undefined);
  const [tries, setTries] = useState(0); // the same wrong password twice still retries
  const { job, error, busy, run, cancel } = useJob();

  useEffect(() => {
    let live = true;
    setFields(null); setValues({}); setLoadError(null);
    listFields(path, password).then(
      (f) => { if (live) setFields(f); },
      (e) => { if (live) setLoadError(String(e)); },
    );
    return () => { live = false; };
  }, [path, password, tries]);

  const changed = (fields ?? [])
    .filter((f) => values[f.name] !== undefined && values[f.name] !== f.value)
    .map((f) => ({ name: f.name, value: values[f.name] }));
  const apply = () => void run(() => callTool("fill-form", { input: path, ...(password ? { password } : {}), values: changed }));
  const need = passwordNeed(loadError);

  return (
    <form onSubmit={(e) => { e.preventDefault(); apply(); }}>
      <h2>Fill Form</h2>
      {loadError && <p role="alert" className="error">{loadError}</p>}
      {need && <PasswordPrompt files={[path]} wrong={need === "wrong"} onSubmit={([p]) => { setPassword(p); setTries((t) => t + 1); }} />}
      {fields?.length === 0 && <p className="muted">This PDF has no form fields.</p>}
      {fields?.map((f, i) => (
        <label key={i} className="field">
          <span>{f.name}</span>
          <Control f={f} id={i} value={values[f.name] ?? f.value} set={(v) => setValues((p) => ({ ...p, [f.name]: v }))} />
        </label>
      ))}
      <button type="submit" disabled={busy || changed.length === 0}>Apply</button>
      <JobStatus job={job} error={error} onCancel={(id) => void cancel(id)} />
    </form>
  );
}

type ControlProps = { f: FormField; id: number; value: string; set: (v: string) => void };

/** Text box, a select of the allowed values, or a box with suggestions (combo); fixed fields are disabled. */
function Control({ f, id, value, set }: ControlProps) {
  const onChange = (e: { target: { value: string } }) => set(e.target.value);
  if (f.read_only || f.kind === "button" || f.kind === "signature") return <input value={value} disabled />;
  const options = f.options ?? [];
  if (f.kind === "combo") {
    return (
      <>
        <input list={`field-options-${id}`} value={value} onChange={onChange} />
        <datalist id={`field-options-${id}`}>{options.map((o) => <option key={o} value={o} />)}</datalist>
      </>
    );
  }
  if (f.kind === "text") return <input value={value} onChange={onChange} />;
  const choices = f.kind === "list" ? options : [...options, "Off"];
  return (
    <select value={value} onChange={onChange}>
      {!choices.includes(value) && <option value={value}>{value || "None"}</option>}
      {choices.map((o) => <option key={o} value={o}>{o}</option>)}
    </select>
  );
}
