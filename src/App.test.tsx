import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

let office = true;
const invoke = vi.fn(async (cmd: string) =>
  cmd === "pages_info" ? { page_count: 1, pages: [{ media_box: [0, 0, 612, 792], crop_box: [0, 0, 612, 792], rotation: 0 }] } : cmd === "mcp_info" ? { status: "starting" }
    : cmd === "forms_list_fields" ? [] : cmd === "office_available" ? office : new ArrayBuffer(4),
);
const openDialog = vi.fn();
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: (...a: unknown[]) => openDialog(...a) }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: [string]) => invoke(...a) }));

let drop: (e: { payload: unknown }) => void = () => {};
const unlisten = vi.fn();
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: (cb: typeof drop) => {
      drop = cb;
      return Promise.resolve(unlisten);
    },
  }),
}));

let openDoc: (e: { payload: string }) => void = () => {};
const unlistenOpen = vi.fn();
vi.mock("@tauri-apps/api/event", () => ({
  listen: (name: string, cb: typeof openDoc) => {
    if (name === "open-document") openDoc = cb;
    return Promise.resolve(unlistenOpen);
  },
}));

import App from "./App";

describe("App", () => {
  it("asks for a drop, then shows the dropped pdf in Organize, ignoring non-pdfs", async () => {
    const { unmount } = render(<App />);
    expect(screen.getByText("Drop a PDF onto the window to start.")).toBeTruthy();
    await act(async () => drop({ payload: { type: "drop", paths: ["/a/x.txt", "/a/Doc.PDF"] } }));
    expect(await screen.findAllByRole("img")).toHaveLength(1);
    expect(invoke).toHaveBeenCalledWith("pages_info", { path: "/a/Doc.PDF" });
    await act(async () => drop({ payload: { type: "over" } }));
    expect(screen.getAllByRole("img")).toHaveLength(1);
    unmount();
    await vi.waitFor(() => expect(unlisten).toHaveBeenCalled());
  });

  it("Open PDF fills the files like a drop; cancel keeps them", async () => {
    render(<App />);
    openDialog.mockResolvedValueOnce("/a/Picked.pdf");
    await userEvent.click(screen.getByRole("button", { name: "Open PDF" }));
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("pages_info", { path: "/a/Picked.pdf" }));
    openDialog.mockResolvedValueOnce(null);
    await userEvent.click(screen.getByRole("button", { name: "Open PDF" }));
    expect(screen.queryByText("Drop a PDF onto the window to start.")).toBeNull();
  });

  it("shows why Open failed", async () => {
    render(<App />);
    openDialog.mockRejectedValueOnce("dialog not allowed");
    await userEvent.click(screen.getByRole("button", { name: "Open PDF" }));
    expect((await screen.findByRole("alert")).textContent).toBe("dialog not allowed");
  });

  it("gives each tool only the dropped files it accepts", async () => {
    render(<App />);
    await act(async () => drop({ payload: { type: "drop", paths: ["/a/p.png", "/a/Doc.pdf", "/a/q.JPG"] } }));
    await userEvent.click(screen.getByRole("button", { name: "Images to PDF" }));
    expect(screen.getByText("/a/p.png, /a/q.JPG")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Rotate Pages" }));
    expect(screen.getByText("/a/Doc.pdf")).toBeTruthy();
  });

  it("switches tool from the nav", async () => {
    render(<App />);
    await userEvent.click(screen.getByRole("button", { name: "Rotate Pages" }));
    expect(screen.getByRole("button", { name: "Rotate Pages" }).getAttribute("aria-current")).toBe("true");
    expect(screen.queryByText("Drop a PDF onto the window to start.")).toBeNull();
  });

  it("shows the dropped form's fields in Fill Form", async () => {
    render(<App />);
    await act(async () => drop({ payload: { type: "drop", paths: ["/a/Form.pdf"] } }));
    await userEvent.click(screen.getByRole("button", { name: "Fill Form" }));
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("forms_list_fields", { path: "/a/Form.pdf" }));
    expect(await screen.findByRole("heading", { name: "Fill Form" })).toBeTruthy();
  });

  it("opens the document an agent sent with pdf_open_in_app in the viewer, leaving Settings", async () => {
    const { unmount } = render(<App />);
    await userEvent.click(screen.getByRole("button", { name: "MCP Settings" }));
    expect(await screen.findByText("MCP server is starting.")).toBeTruthy();
    await act(async () => openDoc({ payload: "/b/Agent.pdf" }));
    expect(screen.queryByText("MCP server is starting.")).toBeNull();
    expect(screen.getByRole("heading", { name: "View & Annotate" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "View & Annotate" }).getAttribute("aria-current")).toBe("true");
    expect(await screen.findByRole("img", { name: "Page 1" })).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("pages_info", { path: "/b/Agent.pdf" });
    unmount();
    await vi.waitFor(() => expect(unlistenOpen).toHaveBeenCalled());
  });

  it("greys out Office to PDF without the bundled WordCraft and says why", async () => {
    const why = "WordCraft not found - the bundled converter is missing.";
    office = false;
    const { unmount } = render(<App />);
    const btn = screen.getByRole("button", { name: "Office to PDF" });
    await vi.waitFor(() => expect(btn.getAttribute("aria-disabled")).toBe("true"));
    expect(btn.getAttribute("title")).toBe(why);
    expect(screen.getByRole("button", { name: "Images to PDF" }).getAttribute("aria-disabled")).toBeNull();
    await userEvent.click(btn);
    expect(screen.getByText(why)).toBeTruthy();
    unmount();
    office = true;
    render(<App />);
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("office_available"));
    const btn2 = screen.getByRole("button", { name: "Office to PDF" });
    await vi.waitFor(() => expect(btn2.getAttribute("aria-disabled")).toBeNull());
    expect(screen.queryByRole("button", { name: "PDF to Office" })).toBeNull();
  });
});
