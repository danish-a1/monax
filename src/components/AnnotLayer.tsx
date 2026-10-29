import { type PointerEvent, useEffect, useState } from "react";
import { bounds, type Drawn, kindOf, type Mark, type Pending, type Tool } from "../lib/annots";
import { fromPdfSpace, type PageBoxes, type Point, toPdfSpace } from "../lib/coords";
import { humanize } from "../tools";

interface Props {
  page: PageBoxes;
  /** Displayed page size, CSS pixels. */
  width: number;
  height: number;
  tool: Tool;
  /** Pending marks on this page. */
  items: Pending[];
  selected: number | null;
  /** A finished drawing, in PDF user space. */
  onDraw: (d: Drawn) => void;
  onSelect: (id: number) => void;
}

/** Below this many pixels a drag is a click, not a rect. */
const MIN_PX = 2;

/** SVG over the page: draws pending marks and turns pointer drags into PDF user space shapes. */
export default function AnnotLayer({ page, width, height, tool, items, selected, onDraw, onSelect }: Props) {
  const [draft, setDraft] = useState<Point[] | null>(null);
  const canvas = { left: 0, top: 0, width, height };
  const toPdf = (p: Point) => { const [x, y] = toPdfSpace(p, page, canvas); return { x, y }; };

  useEffect(() => {
    if (!draft) return;
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") setDraft(null); };
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [draft]);

  const local = (e: PointerEvent<SVGSVGElement>): Point => {
    const r = e.currentTarget.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  };
  const down = (e: PointerEvent<SVGSVGElement>) => {
    if (tool === "select") return;
    e.currentTarget.setPointerCapture?.(e.pointerId);
    setDraft([local(e)]);
  };
  const move = (e: PointerEvent<SVGSVGElement>) => {
    if (!draft) return;
    const p = local(e);
    setDraft(tool === "ink" ? [...draft, p] : [draft[0], p]);
  };
  const up = () => {
    if (!draft) return;
    setDraft(null);
    if (tool === "ink") return onDraw({ points: draft.map(toPdf) });
    const [a, b] = [draft[0], draft[draft.length - 1]];
    if (Math.abs(a.x - b.x) < MIN_PX || Math.abs(a.y - b.y) < MIN_PX) return;
    const [p, q] = [toPdf(a), toPdf(b)];
    onDraw({ rect: { left: Math.min(p.x, q.x), bottom: Math.min(p.y, q.y), right: Math.max(p.x, q.x), top: Math.max(p.y, q.y) } });
  };

  const shown = (m: Mark) => {
    const r = bounds(m);
    const [a, b] = [fromPdfSpace([r.left, r.bottom], page, canvas), fromPdfSpace([r.right, r.top], page, canvas)];
    return { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), width: Math.abs(a.x - b.x), height: Math.abs(a.y - b.y) };
  };
  const line = (pts: Point[]) => pts.map((p) => `${p.x},${p.y}`).join(" ");

  return (
    <svg
      className={tool === "select" ? "annots" : "annots drawing"} width={width} height={height}
      role="group" aria-label="Page annotations" onPointerDown={down} onPointerMove={move} onPointerUp={up}
    >
      {items.map(({ id, mark }, i) => {
        const r = shown(mark);
        const color = "color" in mark ? mark.color : "#000000";
        return (
          <g
            key={id} aria-label={`${humanize(kindOf(mark))} ${i + 1}`} onClick={() => onSelect(id)}
            className={`mark ${mark.type}${id === selected ? " selected" : ""}`} stroke={color} fill={mark.type === "highlight" ? color : "none"}
          >
            {mark.type === "ink" ? (
              mark.strokes.map((s, k) => <polyline key={k} points={line(s.map((p) => fromPdfSpace([p.x, p.y], page, canvas)))} />)
            ) : mark.type === "shape" && mark.kind === "ellipse" ? (
              <ellipse cx={r.x + r.width / 2} cy={r.y + r.height / 2} rx={r.width / 2} ry={r.height / 2} />
            ) : (
              <rect {...r} />
            )}
          </g>
        );
      })}
      {draft && <polyline className="draft" points={line(tool === "ink" ? draft : box(draft[0], draft[draft.length - 1]))} />}
    </svg>
  );
}

/** The closed outline of the rect with corners `a` and `b`. */
const box = (a: Point, b: Point): Point[] => [a, { x: b.x, y: a.y }, b, { x: a.x, y: b.y }, a];
