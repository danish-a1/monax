import { open } from "@tauri-apps/plugin-dialog";
import type { ToolEntry } from "../tools";

type Filter = { name: string; extensions: string[] };
const PDF: Filter[] = [{ name: "PDF", extensions: ["pdf"] }];

/** The open-panel button, filter and selection mode for `tool`'s input files. */
export function pickerFor(tool: ToolEntry): { label: string; filters: Filter[]; multiple: boolean } {
  const multiple = Boolean(tool.multi) || (tool.fileKeys?.length ?? 1) > 1;
  const exts = tool.accepts?.map((e) => e.slice(1));
  if (!exts) return { label: "Open PDF", filters: PDF, multiple };
  const images = tool.id === "images-to-pdf";
  return { label: images ? "Open Images" : "Open Document", filters: [{ name: images ? "Images" : "Documents", extensions: exts }], multiple };
}

/** A native open panel for files; `onPick` gets the chosen paths (never on cancel), `onError` the reason the panel failed.
 * Convenience only: the backend still validates every path. */
export default function PickPath({ label, filters, multiple = false, onPick, onError }: {
  label: string; filters?: Filter[]; multiple?: boolean; onPick: (paths: string[]) => void; onError?: (message: string) => void;
}) {
  async function pick() {
    try {
      const got = await open({ multiple, filters });
      if (got !== null) onPick(Array.isArray(got) ? got : [got]);
    } catch (e) {
      onError?.(e instanceof Error ? e.message : String(e));
    }
  }
  return <button type="button" onClick={() => void pick()}>{label}</button>;
}
