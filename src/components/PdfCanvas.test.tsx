import { render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
import PdfCanvas from "./PdfCanvas";

const renders = () => invoke.mock.calls.filter(([c]) => c === "pages_thumbnail").map(([, a]) => a);

describe("PdfCanvas", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(new ArrayBuffer(4));
    vi.mocked(URL.revokeObjectURL).mockClear();
    vi.stubGlobal("devicePixelRatio", 2);
  });
  afterEach(() => vi.unstubAllGlobals());

  it("renders the page at device pixels once the size settles, and drops the image on unmount", async () => {
    const onError = vi.fn();
    const onRendered = vi.fn();
    const props = { path: "/a/Doc.pdf", page: 1, onError, onRendered };
    const { rerender, unmount } = render(<PdfCanvas {...props} width={300} height={400} />);
    const img = await screen.findByRole("img", { name: "Page 2" });
    expect(img.getAttribute("src")).toBe("blob:thumb");
    expect(renders()).toEqual([{ path: "/a/Doc.pdf", page: 1, max_px: 800 }]);
    expect(onRendered).toHaveBeenCalledTimes(1);
    rerender(<PdfCanvas {...props} width={600} height={800} />);
    rerender(<PdfCanvas {...props} width={3000} height={4000} password="pw" />);
    await vi.waitFor(() => expect(renders()).toHaveLength(2));
    expect(renders()[1]).toEqual({ path: "/a/Doc.pdf", page: 1, max_px: 4096, password: "pw" });
    await vi.waitFor(() => expect(URL.revokeObjectURL).toHaveBeenCalledTimes(1)); // the old image
    unmount();
    expect(URL.revokeObjectURL).toHaveBeenCalledTimes(2);
    expect(onError).not.toHaveBeenCalled();
  });

  it("reports a render failure", async () => {
    invoke.mockRejectedValue("page index 9 out of range (document has 3 pages, indices 0-2)");
    const onError = vi.fn();
    render(<PdfCanvas path="/a/Doc.pdf" page={9} width={100} height={100} onError={onError} />);
    await vi.waitFor(() => expect(onError).toHaveBeenCalledWith("page index 9 out of range (document has 3 pages, indices 0-2)"));
  });
});
