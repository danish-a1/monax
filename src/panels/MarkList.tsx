import { useState } from "react";
import { kindOf, type Pending } from "../lib/annots";
import { humanize } from "../tools";

interface Props {
  items: Pending[];
  selected: number | null;
  busy: boolean;
  onSelect: (id: number) => void;
  onRemove: (id: number) => void;
  /** Saves the annotations (one annotate job). */
  onSave: () => void;
  /** Applies the redaction boxes (one redact job), once confirmed. */
  onRedact: () => void;
}

/** Pending marks, each selectable and removable, and the two ways to apply them. */
export default function MarkList({ items, selected, busy, onSelect, onRemove, onSave, onRedact }: Props) {
  const [confirm, setConfirm] = useState(false);
  const redactions = items.filter((i) => i.mark.type === "redact").length;
  const name = (i: Pending) => `${humanize(kindOf(i.mark))} on page ${i.page + 1}`;
  return (
    <div className="marks">
      <ul aria-label="Pending Marks">
        {items.map((i) => (
          <li key={i.id}>
            <button type="button" aria-pressed={i.id === selected} onClick={() => onSelect(i.id)}>{name(i)}</button>
            <button type="button" aria-label={`Remove ${name(i)}`} onClick={() => onRemove(i.id)}>Remove</button>
          </li>
        ))}
      </ul>
      <div className="toolbar">
        <button type="button" disabled={busy || items.length === redactions} onClick={onSave}>Save Annotations</button>
        <button type="button" disabled={busy || redactions === 0} onClick={() => setConfirm(true)}>Apply Redactions</button>
      </div>
      {confirm && (
        <div role="group" aria-label="Confirm Redaction" className="prompt">
          <p>Each page with a box becomes an image with the boxes blacked out: text on this page is no longer searchable.</p>
          <button type="button" onClick={() => { setConfirm(false); onRedact(); }}>Redact Now</button>
          <button type="button" onClick={() => setConfirm(false)}>Keep Editing</button>
        </div>
      )}
    </div>
  );
}
