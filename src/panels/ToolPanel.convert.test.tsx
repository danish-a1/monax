import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
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
      ? { id: "9", state: { kind: "done" }, progress: 1, outputs: ["/a/x-1.png", "/a/x-2.png"] }
      : { job_id: "9" },
  );
});

describe("convert tools", () => {
  it("asks for images and sends the chosen paper size", async () => {
    const { rerender } = render(<ToolPanel tool={tool("images-to-pdf")} files={[]} />);
    expect(screen.getByText("Drop .png, .jpg, .jpeg files onto the window.")).toBeTruthy();
    rerender(<ToolPanel tool={tool("images-to-pdf")} files={["/a/p.png", "/a/q.jpg"]} />);
    const paper = screen.getByLabelText("Page Size") as HTMLSelectElement;
    expect(paper.value).toBe("same_as_image");
    // A field with a default has no blank "Default" choice.
    expect([...paper.options].map((o) => o.textContent)).toEqual(["Same As Image", "A4", "Letter"]);
    await userEvent.selectOptions(paper, "A4");
    await userEvent.selectOptions(screen.getByLabelText("Orientation"), "Landscape");
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("convert_images_to_pdf", {
        input: { inputs: ["/a/p.png", "/a/q.jpg"], page_size: { width: 841.89, height: 595.28 } },
      }),
    );
  });

  it("renders pages to images and lists every file", async () => {
    render(<ToolPanel tool={tool("pdf-to-images")} files={["/a/x.pdf"]} />);
    await userEvent.selectOptions(screen.getByLabelText("Format"), "Jpeg");
    await userEvent.clear(screen.getByLabelText("Resolution (DPI)"));
    await userEvent.type(screen.getByLabelText("Resolution (DPI)"), "300");
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    expect(await screen.findByText("/a/x-2.png")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("convert_pdf_to_images", {
      input: { input: "/a/x.pdf", dpi: 300, format: "jpeg", jpeg_quality: 85 },
    });
  });

  it("shows why an Office tool cannot run and keeps Apply off", () => {
    const why = "WordCraft not found - the bundled converter is missing.";
    render(<ToolPanel tool={tool("office-to-pdf")} files={["/a/r.docx"]} blocked={why} />);
    expect(screen.getByText(why)).toBeTruthy();
    expect((screen.getByRole("button", { name: "Apply" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("converts a dropped Markdown file to PDF", async () => {
    const { rerender } = render(<ToolPanel tool={tool("office-to-pdf")} files={[]} />);
    const hint = "Drop .docx, .doc, .odt, .rtf, .txt, .html, .htm, .md files onto the window.";
    expect(screen.getByText(hint)).toBeTruthy();
    rerender(<ToolPanel tool={tool("office-to-pdf")} files={["/a/n.md"]} />);
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("convert_office_to_pdf", { input: { input: "/a/n.md" } }));
  });
});
