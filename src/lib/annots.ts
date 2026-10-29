import type { Point, Rect } from "./coords";

/** Viewer tools: select, the core's annotation kinds, and a redaction box. */
export const TOOLS = ["select", "highlight", "underline", "strikeout", "note", "free_text", "ink", "rectangle", "ellipse", "redact"] as const;
export type Tool = (typeof TOOLS)[number];

/** The core's `Annot` (serde tag `type`); rects and points in PDF user space. */
export type Annot =
  | { type: "highlight" | "underline" | "strikeout"; rect: Rect; color: string }
  | { type: "note"; rect: Rect; text: string }
  | { type: "free_text"; rect: Rect; text: string; font_size: number; color: string }
  | { type: "ink"; strokes: Point[][]; width: number; color: string }
  | { type: "shape"; kind: "rectangle" | "ellipse"; rect: Rect; color: string; width: number };
/** What a tool leaves on a page before Save: an annotation, or a box the redact op blacks out. */
export type Mark = Annot | { type: "redact"; rect: Rect };
export interface Pending { id: number; page: number; mark: Mark }
/** Toolbar settings a new mark takes: color `#RRGGBB`, line width and font size in points. */
export interface Style { color: string; width: number; text: string; fontSize: number }
/** What was drawn, in PDF user space: a dragged rect, or an ink stroke's points. */
export type Drawn = { rect: Rect } | { points: Point[] };

/** The mark `tool` makes of `drawn`, or why it cannot (a message for the user). */
export function makeMark(tool: Exclude<Tool, "select">, drawn: Drawn, style: Style): Mark | string {
  const { color, width, text } = style;
  if ("points" in drawn) return drawn.points.length < 2 ? "Draw a longer line." : { type: "ink", strokes: [drawn.points], width, color };
  const { rect } = drawn;
  if ((tool === "note" || tool === "free_text") && !text.trim()) return "Type the text first.";
  switch (tool) {
    case "note": return { type: "note", rect, text };
    case "free_text": return { type: "free_text", rect, text, font_size: style.fontSize, color };
    case "rectangle": case "ellipse": return { type: "shape", kind: tool, rect, color, width };
    case "redact": return { type: "redact", rect };
    case "ink": return "Draw a longer line.";
    default: return { type: tool, rect, color };
  }
}

/** A mark's extent in PDF user space (an ink stroke: its points' bounds). */
export function bounds(m: Mark): Rect {
  if (m.type !== "ink") return m.rect;
  const xs = m.strokes.flat().map((p) => p.x);
  const ys = m.strokes.flat().map((p) => p.y);
  return { left: Math.min(...xs), bottom: Math.min(...ys), right: Math.max(...xs), top: Math.max(...ys) };
}

/** The mark's kind for labels: "rectangle" for a rectangle shape. */
export const kindOf = (m: Mark) => (m.type === "shape" ? m.kind : m.type);
