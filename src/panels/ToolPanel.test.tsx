import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import ToolPanel from "./ToolPanel";
import { TOOLS } from "../tools";

const tool = (id: string) => TOOLS.find((t) => t.id === id)!;

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(async (cmd: string) =>
    cmd === "job_status"
      ? { id: "9", state: { kind: "done" }, progress: 1, outputs: ["/a/x-rotate.pdf"] }
      : { job_id: "9" },
  );
});

describe("ToolPanel", () => {
  it("renders labeled inputs from FieldSpec", () => {
    render(<ToolPanel tool={tool("rotate")} files={["/a/x.pdf"]} />);
    expect(screen.getByLabelText("Pages")).toBeTruthy();
    expect(screen.getByLabelText("Degrees")).toBeTruthy();
  });

  it("calls the tool command and shows outputs", async () => {
    render(<ToolPanel tool={tool("rotate")} files={["/a/x.pdf"]} />);
    await userEvent.type(screen.getByLabelText("Pages"), "1-2, 4");
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("pages_rotate", {
        input: { input: "/a/x.pdf", pages: [0, 1, 3], degrees: 90 },
      }),
    );
    expect(await screen.findByText("/a/x-rotate.pdf")).toBeTruthy();
  });

  it("shows a finished job's notes apart from errors", async () => {
    const note = "1 image left above 72 dpi: color space not gray or RGB";
    invoke.mockImplementation(async (cmd: string) =>
      cmd === "job_status"
        ? { id: "9", state: { kind: "done" }, progress: 1, outputs: ["/a/x-compress.pdf"], notes: [note] }
        : { job_id: "9" },
    );
    render(<ToolPanel tool={tool("compress")} files={["/a/x.pdf"]} />);
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    const notes = await screen.findByRole("list", { name: "Notes" });
    expect(notes.textContent).toBe(note);
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByText("/a/x-compress.pdf")).toBeTruthy();
  });

  it("shows a tool's hint and sends redaction boxes", async () => {
    render(<ToolPanel tool={tool("redact")} files={["/a/x.pdf"]} />);
    expect(screen.getByText(/no longer searchable/)).toBeTruthy();
    await userEvent.type(screen.getByLabelText("Pages"), "2");
    for (const [k, v] of [["Left", "1"], ["Bottom", "2"], ["Right", "3"], ["Top", "4"]])
      await userEvent.type(screen.getByLabelText(`${k} (pt)`), v);
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("security_redact", {
        input: { input: "/a/x.pdf", boxes: [{ page: 1, rect: { left: 1, bottom: 2, right: 3, top: 4 } }] },
      }),
    );
  });

  it("says signing is not a digital signature and sends one page", async () => {
    const user = userEvent.setup({ delay: null }); // many keystrokes: no per-key delay
    render(<ToolPanel tool={tool("sign-text")} files={["/a/x.pdf"]} />);
    expect(screen.getByText(/not a digital \(cryptographic\) signature/)).toBeTruthy();
    await user.type(screen.getByLabelText("Signature Text"), "Ann");
    await user.type(screen.getByLabelText("Page"), "1-2");
    for (const [k, v] of [["Left", "1"], ["Bottom", "2"], ["Right", "3"], ["Top", "4"]])
      await user.type(screen.getByLabelText(`${k} (pt)`), v);
    await user.click(screen.getByRole("button", { name: "Apply" }));
    expect(await screen.findByText(/Enter one page/)).toBeTruthy();
    await user.clear(screen.getByLabelText("Page"));
    await user.type(screen.getByLabelText("Page"), "2");
    await user.click(screen.getByRole("button", { name: "Apply" }));
    const placement = { page: 1, rect: { left: 1, bottom: 2, right: 3, top: 4 }, rotate: 0, opacity: 1 };
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("forms_sign_text", { input: { input: "/a/x.pdf", text: "Ann", font_size: 24, placement } }),
    );
  });

  it("disables Apply without a file", () => {
    render(<ToolPanel tool={tool("rotate")} files={[]} />);
    expect((screen.getByRole("button", { name: "Apply" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("cancels a running job and surfaces a cancel failure", async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === "job_status") return { id: "9", state: { kind: "running" }, progress: 0.2, outputs: [] };
      if (cmd === "job_cancel") throw "cancel refused";
      return { job_id: "9" };
    });
    render(<ToolPanel tool={tool("rotate")} files={["/a/x.pdf"]} />);
    await userEvent.type(screen.getByLabelText("Pages"), "1");
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    await userEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(invoke).toHaveBeenCalledWith("job_cancel", { job_id: "9" });
    expect((await screen.findByRole("alert")).textContent).toContain("cancel refused");
  });

  it("shows an IPC error from job_status, not 'Job not found'", async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === "job_status") throw "backend down";
      return { job_id: "9" };
    });
    render(<ToolPanel tool={tool("rotate")} files={["/a/x.pdf"]} />);
    await userEvent.type(screen.getByLabelText("Pages"), "1");
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    expect((await screen.findByRole("alert")).textContent).toBe("backend down");
  });

  it("renders text, select and color fields and sends opts", async () => {
    render(<ToolPanel tool={tool("watermark")} files={["/a/x.pdf"]} />);
    await userEvent.type(screen.getByLabelText("Text"), "DRAFT");
    await userEvent.selectOptions(screen.getByLabelText("Position"), "Top Right");
    const color = screen.getByLabelText("Color") as HTMLInputElement;
    expect(color.type).toBe("color");
    fireEvent.input(color, { target: { value: "#00ff00" } });
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("content_watermark", {
        input: { input: "/a/x.pdf", opts: { text: "DRAFT", position: "top_right", color: [0, 255, 0] } },
      }),
    );
  });

  it("shows the core default as the select's blank choice", () => {
    render(<ToolPanel tool={tool("page-numbers")} files={["/a/x.pdf"]} />);
    const sel = screen.getByLabelText("Position") as HTMLSelectElement;
    expect(sel.value).toBe("");
    expect(sel.options[0].textContent).toBe("Default");
    expect(sel.options).toHaveLength(10);
  });

  it("accepts fractional numbers", async () => {
    render(<ToolPanel tool={tool("watermark")} files={["/a/x.pdf"]} />);
    const opacity = screen.getByLabelText("Opacity") as HTMLInputElement;
    await userEvent.type(opacity, "0.3");
    expect(opacity.step).toBe("any");
    expect(opacity.checkValidity()).toBe(true);
  });

  it("lists every output of a multi-output op", async () => {
    invoke.mockImplementation(async (cmd: string) =>
      cmd === "job_status"
        ? { id: "9", state: { kind: "done" }, progress: 1, outputs: ["/a/x-1.pdf", "/a/x-2.pdf"] }
        : { job_id: "9" },
    );
    render(<ToolPanel tool={tool("split-pages")} files={["/a/x.pdf"]} />);
    expect((screen.getByLabelText("Ranges") as HTMLInputElement).placeholder).toBe("1-3, 5");
    await userEvent.type(screen.getByLabelText("Ranges"), "1, 2-3");
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    expect(await screen.findByText("/a/x-1.pdf")).toBeTruthy();
    expect(screen.getByText("/a/x-2.pdf")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("pages_split_pages", {
      input: { input: "/a/x.pdf", ranges: [{ start: 0, end: 0 }, { start: 1, end: 2 }] },
    });
  });

  it("offers the installed OCR languages and sends the chosen one", async () => {
    invoke.mockImplementation(async (cmd: string) =>
      cmd === "ocr_languages" ? ["deu", "eng"]
        : cmd === "job_status" ? { id: "9", state: { kind: "done" }, progress: 1, outputs: ["/a/x-ocr.pdf"] }
          : { job_id: "9" },
    );
    render(<ToolPanel tool={tool("ocr")} files={["/a/x.pdf"]} />);
    const sel = screen.getByLabelText("Language") as HTMLSelectElement;
    await waitFor(() => expect(sel.options).toHaveLength(3));
    expect(invoke).toHaveBeenCalledWith("ocr_languages");
    await userEvent.selectOptions(sel, "Deu");
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("ocr_run", { input: { input: "/a/x.pdf", lang: ["deu"], dpi: 300 } }),
    );
  });

  it("needs both files for alternate mix", () => {
    const { rerender } = render(<ToolPanel tool={tool("alternate-mix")} files={[]} />);
    expect(screen.getByText("Drop 2 PDFs onto the window.")).toBeTruthy();
    const apply = () => screen.getByRole("button", { name: "Apply" }) as HTMLButtonElement;
    rerender(<ToolPanel tool={tool("alternate-mix")} files={["/a.pdf"]} />);
    expect(apply().disabled).toBe(true);
    rerender(<ToolPanel tool={tool("alternate-mix")} files={["/a.pdf", "/b.pdf"]} />);
    expect(apply().disabled).toBe(false);
  });
});
