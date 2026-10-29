import { fireEvent, screen } from "@testing-library/react";
import type { Mock } from "vitest";

// Shared by the Viewer test files (each mocks @tauri-apps/api/core with its own `invoke`).
// Letter pages; page 2 is /Rotate 90. Drawn at 100% = 4/3 CSS px per point.
const letter = { media_box: [0, 0, 612, 792], crop_box: [0, 0, 612, 792], rotation: 0 };
export const INFO = { page_count: 3, pages: [letter, { ...letter, rotation: 90 }, letter] };
export const DONE = { id: "5", state: { kind: "done" }, progress: 1, outputs: ["/a/Doc-out.pdf"] };

type Args = { password?: string; page?: number };
/** Answers the viewer's commands; `info` and `thumb` may throw a backend message. */
export function backend(invoke: Mock, info: (a: Args) => unknown = () => INFO, thumb: (a: Args) => unknown = () => new ArrayBuffer(4)) {
  invoke.mockImplementation(async (cmd: string, args: Args) => {
    if (cmd === "pages_info") return info(args);
    if (cmd === "pages_thumbnail") return thumb(args);
    if (cmd === "annotate_save" || cmd === "security_redact") return { job_id: "5" };
    if (cmd === "job_status") return DONE;
    throw new Error(`unexpected ${cmd}`);
  });
}
/** Arguments of every call to `cmd`. */
export const sent = (invoke: Mock, cmd: string) => invoke.mock.calls.filter(([c]) => c === cmd).map(([, a]) => a);
export const layer = () => screen.getByRole("group", { name: "Page annotations" });
export function drag(x0: number, y0: number, x1: number, y1: number) {
  fireEvent.pointerDown(layer(), { clientX: x0, clientY: y0, pointerId: 1 });
  fireEvent.pointerMove(layer(), { clientX: x1, clientY: y1, pointerId: 1 });
  fireEvent.pointerUp(layer(), { clientX: x1, clientY: y1, pointerId: 1 });
}
