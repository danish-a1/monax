import { describe, expect, it } from "vitest";
import { OFFICE_MISSING } from "./convertTools";
import { TOOLS, acceptedFiles, buildInput } from "./tools";

const tool = (id: string) => TOOLS.find((t) => t.id === id)!;

describe("office tools", () => {
  it("need the bundled WordCraft and say so in exactly R152's words", () => {
    expect(OFFICE_MISSING).toBe("WordCraft not found - the bundled converter is missing.");
    const office = TOOLS.filter((t) => t.requires === "office").map((t) => t.id);
    expect(office).toEqual(["office-to-pdf"]);
    expect(TOOLS.filter((t) => t.group === "Convert").map((t) => t.title)).toEqual(
      ["Extract Images", "Images to PDF", "PDF to Images", "PDF to Text", "Office to PDF"]);
  });

  it("take Word, OpenDocument, RTF, text, HTML and Markdown files and send the input", () => {
    const t = tool("office-to-pdf");
    expect(t.accepts).toEqual([".docx", ".doc", ".odt", ".rtf", ".txt", ".html", ".htm", ".md"]);
    const files = ["/a/r.DOCX", "/a/s.xlsx", "/a/p.pdf", "/a/n.pages", "/a/t.txt", "/a/d.csv", "/a/o.odp",
      "/a/w.HTM", "/a/m.md", "/a/x.pptx"];
    expect(acceptedFiles(t, files)).toEqual(["/a/r.DOCX", "/a/t.txt", "/a/w.HTM", "/a/m.md"]);
    expect(buildInput(t, ["/a/r.docx"], {})).toEqual({ input: "/a/r.docx" });
  });
});
