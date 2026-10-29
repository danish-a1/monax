import { useState } from "react";
import { ENCRYPTED, WRONG_PASSWORD } from "../lib/invoke";

/** The failure asks for a password: "encrypted", "wrong", or null when it does not. */
export function passwordNeed(message: string | null | undefined): "encrypted" | "wrong" | null {
  return message === ENCRYPTED ? "encrypted" : message === WRONG_PASSWORD ? "wrong" : null;
}

interface Props { files: string[]; wrong: boolean; onSubmit: (passwords: string[]) => void }

const base = (p: string) => p.split("/").pop() ?? p;

/** One masked field per file; the typed passwords are handed over and cleared on submit. */
export default function PasswordPrompt({ files, wrong, onSubmit }: Props) {
  const [values, setValues] = useState<string[]>(files.map(() => ""));
  const submit = () => {
    const v = values;
    setValues(files.map(() => ""));
    onSubmit(v);
  };
  return (
    <div role="group" aria-label="Password required" className="prompt">
      <p>{wrong ? "Wrong password, try again." : "This PDF is password-protected."}</p>
      {files.map((f, i) => (
        <label key={f + i} className="field">
          <span>{`Password for ${base(f)}`}</span>
          <input
            type="password" autoComplete="off" value={values[i] ?? ""}
            onChange={(e) => setValues((cur) => cur.map((x, j) => (j === i ? e.target.value : x)))}
          />
        </label>
      ))}
      <button type="button" onClick={submit}>Unlock and Retry</button>
    </div>
  );
}
