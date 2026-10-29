// Command input built from a tool's form: page text parsing, field conversion, nesting.
import type { FieldSpec, ToolEntry } from "../tools";

/** "1-3, 5" (1-based) -> [{start: 0, end: 2}, {start: 4, end: 4}]. Core validates against the page count. */
export function parseRanges(text: string): { start: number; end: number }[] {
  const out = text.split(",").map((s) => s.trim()).filter(Boolean).map((part) => {
    const [a, b = a] = part.split("-").map((s) => Number.parseInt(s, 10));
    if (!Number.isInteger(a) || !Number.isInteger(b) || a < 1 || b < a) throw new Error(`Invalid page range: ${part}`);
    return { start: a - 1, end: b - 1 };
  });
  if (out.length === 0) throw new Error("Enter at least one page");
  return out;
}

/** "1-3, 5" (1-based) -> [0, 1, 2, 4]. Viewport-free text parsing only; core validates ranges. */
export function parsePages(text: string): number[] {
  return parseRanges(text).flatMap(({ start, end }) => Array.from({ length: end - start + 1 }, (_, i) => start + i));
}

export type Values = Record<string, string | number | boolean>;

function convert(f: FieldSpec, raw: string | number | boolean): unknown {
  switch (f.kind) {
    case "pages": return parsePages(String(raw));
    case "ranges": return parseRanges(String(raw));
    case "number": return Number(raw);
    case "checkbox": return Boolean(raw);
    case "color": return [1, 3, 5].map((i) => Number.parseInt(String(raw).slice(i, i + 2), 16));
    default: return String(raw);
  }
}

function setPath(o: Record<string, unknown>, path: string, value: unknown) {
  const keys = path.split(".");
  const last = keys.pop()!;
  for (const k of keys) o = (o[k] ??= {}) as Record<string, unknown>;
  o[last] = value;
}

/** Builds the command input from dropped files, form values and the passwords of the files, in order ("" = none). */
export function buildInput(tool: ToolEntry, files: string[], v: Values, passwords: string[] = []): object {
  const built = buildFields(tool, files, v, passwords);
  return tool.shape ? tool.shape(built) : built;
}

function buildFields(tool: ToolEntry, files: string[], v: Values, passwords: string[]): Record<string, unknown> {
  const base = tool.fileKeys
    ? Object.fromEntries(tool.fileKeys.map((k, i) => [k, files[i]]))
    : tool.multi ? { inputs: files } : { input: files[0] };
  const extra: Record<string, unknown> = {};
  const top: Record<string, unknown> = {};
  if (passwords.some((p) => p !== "")) {
    if (tool.multi) top.passwords = files.map((_, i) => passwords[i] || null);
    else top.password = passwords[0];
  }
  for (const f of tool.options) {
    const raw = v[f.name] ?? f.default ?? "";
    const blank = f.kind === "password" ? raw === "" : String(raw).trim() === "";
    if (blank && ((f.kind !== "pages" && f.kind !== "ranges") || f.optional)) continue;
    setPath(extra, f.name, convert(f, raw));
  }
  return tool.optsKey ? { ...base, ...top, [tool.optsKey]: extra } : { ...base, ...top, ...extra };
}

/** Sign tools take one page: the pages field's list becomes its single 0-based index. */
export function onePage({ placement, ...rest }: Record<string, unknown>): object {
  const p = placement as { page: number[] };
  if (p.page.length !== 1) throw new Error("Enter one page");
  return { ...rest, placement: { ...p, page: p.page[0] } };
}

/** Portrait paper sizes, points. */
const PAPER: Record<string, [number, number]> = { a4: [595.28, 841.89], letter: [612, 792] };

/** Images to PDF: paper + orientation -> `page_size` in points; "same as image" sends none. */
export function pageSize({ paper, orientation, ...rest }: Record<string, unknown>): object {
  const size = PAPER[String(paper)];
  if (!size) return rest;
  const [width, height] = orientation === "landscape" ? [size[1], size[0]] : size;
  return { ...rest, page_size: { width, height } };
}

/** The dropped `files` whose extension `tool` accepts (PDFs unless it says otherwise). */
export function acceptedFiles(tool: ToolEntry, files: string[]): string[] {
  const exts = tool.accepts ?? [".pdf"];
  return files.filter((f) => exts.some((e) => f.toLowerCase().endsWith(e)));
}
