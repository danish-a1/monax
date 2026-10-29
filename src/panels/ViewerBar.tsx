import { type Style, type Tool, TOOLS } from "../lib/annots";
import { humanize } from "../tools";

/** Zoom steps, as a fraction of 100% (4/3 CSS pixels per point). */
export const ZOOM_STEPS = [0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4];
/** The next step past `z` in `dir` (+1 in, -1 out), held at 50% and 400%. */
export function step(z: number, dir: 1 | -1): number {
  const s = dir > 0 ? ZOOM_STEPS.find((x) => x > z + 1e-6) : [...ZOOM_STEPS].reverse().find((x) => x < z - 1e-6);
  return s ?? (dir > 0 ? 4 : 0.5);
}

export const toolLabel = (t: Tool) => (t === "redact" ? "Redaction Box" : humanize(t));

interface Props {
  page: number;
  count: number;
  onPage: (page: number) => void;
  zoom: number;
  onZoom: (zoom: number | null) => void;
  tool: Tool;
  onTool: (tool: Tool) => void;
  style: Style;
  onStyle: (style: Style) => void;
}

/** Page navigation, zoom, the drawing tool and what new marks look like. */
export default function ViewerBar({ page, count, onPage, zoom, onZoom, tool, onTool, style, onStyle }: Props) {
  const set = (k: keyof Style, v: string | number) => onStyle({ ...style, [k]: v });
  const num = (k: "width" | "fontSize", label: string) => (
    <label className="field inline">
      <span>{label}</span>
      <input type="number" min={0.25} step={0.25} value={style[k]} onChange={(e) => set(k, Number(e.target.value))} />
    </label>
  );
  return (
    <div className="viewer-bar">
      <div className="toolbar">
        <button type="button" disabled={page === 0} onClick={() => onPage(page - 1)}>Previous Page</button>
        <span>{`Page ${page + 1} of ${count}`}</span>
        <button type="button" disabled={page >= count - 1} onClick={() => onPage(page + 1)}>Next Page</button>
        <button type="button" onClick={() => onZoom(step(zoom, -1))}>Zoom Out</button>
        <output aria-label="Zoom">{`${Math.round(zoom * 100)}%`}</output>
        <button type="button" onClick={() => onZoom(step(zoom, 1))}>Zoom In</button>
        <button type="button" onClick={() => onZoom(null)}>Fit Width</button>
      </div>
      <div className="toolbar" role="group" aria-label="Drawing Tool">
        {TOOLS.map((t) => (
          <button key={t} type="button" aria-pressed={t === tool} onClick={() => onTool(t)}>{toolLabel(t)}</button>
        ))}
      </div>
      <div className="toolbar">
        <label className="field inline">
          <span>Color</span>
          <input type="color" value={style.color} onChange={(e) => set("color", e.target.value)} />
        </label>
        {num("width", "Line Width (pt)")}
        {num("fontSize", "Font Size (pt)")}
        <label className="field inline">
          <span>Text (Note, Free Text)</span>
          <textarea rows={2} value={style.text} onChange={(e) => set("text", e.target.value)} />
        </label>
      </div>
    </div>
  );
}
