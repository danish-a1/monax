import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import Viewer from "./Viewer";

import { backend as answer, drag, layer, sent as calls } from "./viewerTestKit";

const backend = (info?: (a: { password?: string }) => unknown) => answer(invoke, info);
const sent = (cmd: string) => calls(invoke, cmd)[0];

beforeEach(() => { invoke.mockReset(); });

describe("Viewer", () => {
  it("shows a page and changes page with buttons, and with keys in the viewer outside text fields", async () => {
    backend();
    render(<Viewer path="/a/Doc.pdf" />);
    const view = screen.getByRole("region", { name: "Page View" });
    expect(await screen.findByRole("img", { name: "Page 1" })).toBeTruthy();
    expect(screen.getByText("Page 1 of 3")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Previous Page" })).toHaveProperty("disabled", true);
    await userEvent.click(screen.getByRole("button", { name: "Next Page" }));
    expect(await screen.findByRole("img", { name: "Page 2" })).toBeTruthy();
    expect(layer().getAttribute("width")).toBe("1056"); // 792 pt across, rotated
    const outside = document.body.appendChild(document.createElement("button"));
    fireEvent.keyDown(window, { key: "PageDown" });
    fireEvent.keyDown(outside, { key: "PageDown" }); // another panel's control
    fireEvent.keyDown(screen.getByLabelText("Text (Note, Free Text)"), { key: "ArrowDown" });
    expect(screen.getByText("Page 2 of 3")).toBeTruthy();
    outside.remove();
    fireEvent.keyDown(document.body, { key: "PageDown" }); // nothing focused
    expect(screen.getByText("Page 3 of 3")).toBeTruthy();
    fireEvent.keyDown(screen.getByRole("button", { name: "Zoom In" }), { key: "ArrowLeft" }); // a toolbar button
    expect(screen.getByText("Page 2 of 3")).toBeTruthy();
    fireEvent.keyDown(layer(), { key: "ArrowRight" });
    expect(screen.getByText("Page 3 of 3")).toBeTruthy();
    fireEvent.keyDown(view, { key: "PageUp" });
    fireEvent.keyDown(layer(), { key: "ArrowLeft" });
    expect(screen.getByText("Page 1 of 3")).toBeTruthy();
  });

  it("zooms in steps between 50% and 400%, and back to fit width", async () => {
    backend();
    render(<Viewer path="/a/Doc.pdf" />);
    await screen.findByRole("img", { name: "Page 1" });
    expect(screen.getByText("100%")).toBeTruthy(); // fit width without a measured width
    await userEvent.click(screen.getByRole("button", { name: "Zoom In" }));
    expect(screen.getByText("125%")).toBeTruthy();
    expect(layer().getAttribute("width")).toBe("1020");
    for (let i = 0; i < 9; i++) await userEvent.click(screen.getByRole("button", { name: "Zoom In" }));
    expect(screen.getByText("400%")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Fit Width" }));
    for (let i = 0; i < 9; i++) await userEvent.click(screen.getByRole("button", { name: "Zoom Out" }));
    expect(screen.getByText("50%")).toBeTruthy();
  });

  it.each([[100000, "400%"], [10, "50%"], [1020, "125%"]])("fits a %i px wide view at %s, held to 50%..400%", async (px, label) => {
    const width = vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(px);
    backend();
    render(<Viewer path="/a/Doc.pdf" />);
    await screen.findByRole("img", { name: "Page 1" });
    expect(screen.getByText(label)).toBeTruthy();
    width.mockRestore();
  });

  it("lists drawn marks, removes the selected one with Delete, and saves the rest in one job", async () => {
    backend();
    render(<Viewer path="/a/Doc.pdf" />);
    await screen.findByRole("img", { name: "Page 1" });
    await userEvent.click(screen.getByRole("button", { name: "Highlight" }));
    drag(0, 0, 408, 528); // 306 x 396 pt from the top-left
    await userEvent.click(screen.getByRole("button", { name: "Free Text" }));
    drag(0, 0, 100, 100);
    expect(screen.getByText("Type the text first.")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Rectangle" }));
    drag(0, 0, 40, 40);
    await userEvent.click(screen.getByRole("button", { name: "Rectangle on page 1" }));
    fireEvent.keyDown(screen.getByLabelText("Text (Note, Free Text)"), { key: "Delete" });
    expect(screen.getByRole("button", { name: "Rectangle on page 1" })).toBeTruthy();
    fireEvent.keyDown(document.body, { key: "Delete" });
    expect(screen.queryByRole("button", { name: "Rectangle on page 1" })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Save Annotations" }));
    expect(await screen.findByText("Done")).toBeTruthy();
    expect(sent("annotate_save")).toEqual({
      input: {
        input: "/a/Doc.pdf",
        annots: [{ page: 0, annot: { type: "highlight", rect: { left: 0, bottom: 396, right: 306, top: 792 }, color: "#ffd60a" } }],
      },
    });
  });

  it("redacts drawn boxes only after the confirm", async () => {
    backend();
    render(<Viewer path="/a/Doc.pdf" />);
    await screen.findByRole("img", { name: "Page 1" });
    expect(screen.getByRole("button", { name: "Apply Redactions" })).toHaveProperty("disabled", true);
    await userEvent.click(screen.getByRole("button", { name: "Redaction Box" }));
    drag(0, 0, 408, 528);
    await userEvent.click(screen.getByRole("button", { name: "Apply Redactions" }));
    expect(screen.getByText(/text on this page is no longer searchable/)).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Keep Editing" }));
    expect(sent("security_redact")).toBeUndefined();
    await userEvent.click(screen.getByRole("button", { name: "Apply Redactions" }));
    await userEvent.click(screen.getByRole("button", { name: "Redact Now" }));
    expect(await screen.findByText("Done")).toBeTruthy();
    expect(sent("security_redact")).toEqual({
      input: { input: "/a/Doc.pdf", boxes: [{ page: 0, rect: { left: 0, bottom: 396, right: 306, top: 792 } }] },
    });
    expect(screen.getByRole("button", { name: "Save Annotations" })).toHaveProperty("disabled", true);
  });

  it("gives new marks the toolbar's color, width, font size and text, and Remove drops one", async () => {
    backend();
    render(<Viewer path="/a/Doc.pdf" />);
    await screen.findByRole("img", { name: "Page 1" });
    fireEvent.input(screen.getByLabelText("Color"), { target: { value: "#ff0000" } });
    fireEvent.change(screen.getByLabelText("Line Width (pt)"), { target: { value: "3" } });
    fireEvent.change(screen.getByLabelText("Font Size (pt)"), { target: { value: "18" } });
    await userEvent.type(screen.getByLabelText("Text (Note, Free Text)"), "Hi");
    await userEvent.click(screen.getByRole("button", { name: "Free Text" }));
    drag(0, 0, 408, 528);
    await userEvent.click(screen.getByRole("button", { name: "Ellipse" }));
    drag(0, 0, 408, 528);
    await userEvent.click(screen.getByRole("button", { name: "Note" }));
    drag(0, 0, 40, 40);
    await userEvent.click(screen.getByRole("button", { name: "Remove Note on page 1" }));
    await userEvent.click(screen.getByRole("button", { name: "Save Annotations" }));
    await screen.findByText("Done");
    const rect = { left: 0, bottom: 396, right: 306, top: 792 };
    expect((sent("annotate_save") as { input: { annots: unknown[] } }).input.annots).toEqual([
      { page: 0, annot: { type: "free_text", rect, text: "Hi", font_size: 18, color: "#ff0000" } },
      { page: 0, annot: { type: "shape", kind: "ellipse", rect, color: "#ff0000", width: 3 } },
    ]);
  });
});
