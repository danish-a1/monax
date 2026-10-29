import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import PickPath, { pickerFor } from "./PickPath";
import { TOOLS } from "../tools";

const open = vi.fn();
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: (...a: unknown[]) => open(...a) }));

beforeEach(() => open.mockReset());

const PDF = [{ name: "PDF", extensions: ["pdf"] }];

describe("PickPath", () => {
  it("fills the field with the picked path", async () => {
    open.mockResolvedValue("/a/x.pdf");
    const onPick = vi.fn();
    render(<PickPath label="Open PDF" filters={PDF} onPick={onPick} />);
    await userEvent.click(screen.getByRole("button", { name: "Open PDF" }));
    expect(open).toHaveBeenCalledWith({ multiple: false, filters: PDF });
    expect(onPick).toHaveBeenCalledWith(["/a/x.pdf"]);
  });

  it("leaves the field alone on cancel", async () => {
    open.mockResolvedValue(null);
    const onPick = vi.fn();
    render(<PickPath label="Open PDF" filters={PDF} onPick={onPick} />);
    await userEvent.click(screen.getByRole("button", { name: "Open PDF" }));
    expect(onPick).not.toHaveBeenCalled();
  });

  it("passes every path of a multiple selection", async () => {
    open.mockResolvedValue(["/a/1.pdf", "/a/2.pdf"]);
    const onPick = vi.fn();
    render(<PickPath label="Open PDF" filters={PDF} multiple onPick={onPick} />);
    await userEvent.click(screen.getByRole("button", { name: "Open PDF" }));
    expect(open).toHaveBeenCalledWith({ multiple: true, filters: PDF });
    expect(onPick).toHaveBeenCalledWith(["/a/1.pdf", "/a/2.pdf"]);
  });

  it("reports why the open panel failed", async () => {
    open.mockRejectedValueOnce("dialog not allowed");
    const onPick = vi.fn();
    const onError = vi.fn();
    render(<PickPath label="Open PDF" filters={PDF} onPick={onPick} onError={onError} />);
    await userEvent.click(screen.getByRole("button", { name: "Open PDF" }));
    await vi.waitFor(() => expect(onError).toHaveBeenCalledWith("dialog not allowed"));
    expect(onPick).not.toHaveBeenCalled();
  });
});

describe("pickerFor", () => {
  const tool = (id: string) => TOOLS.find((t) => t.id === id)!;
  it("PDF tools open PDFs, one file", () => {
    expect(pickerFor(tool("rotate"))).toEqual({ label: "Open PDF", filters: PDF, multiple: false });
  });
  it("merge takes several PDFs", () => {
    expect(pickerFor(tool("merge")).multiple).toBe(true);
  });
  it("images to PDF takes several images", () => {
    expect(pickerFor(tool("images-to-pdf"))).toEqual({
      label: "Open Images", filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg"] }], multiple: true });
  });
  it("office to PDF takes one document", () => {
    expect(pickerFor(tool("office-to-pdf"))).toEqual({
      label: "Open Document",
      filters: [{ name: "Documents", extensions: ["docx", "doc", "odt", "rtf", "txt", "html", "htm", "md"] }], multiple: false });
  });
});
