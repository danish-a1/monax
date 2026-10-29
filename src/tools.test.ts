import { describe, expect, it } from "vitest";
import { TOOLS, acceptedFiles, buildInput, humanize, parseRanges } from "./tools";

const tool = (id: string) => TOOLS.find((t) => t.id === id)!;

describe("buildInput", () => {
  it("nests options under optsKey (merge)", () => {
    expect(buildInput(tool("merge"), ["/a.pdf", "/b.pdf"], { blank_before_even: true }))
      .toEqual({ inputs: ["/a.pdf", "/b.pdf"], opts: { blank_before_even: true } });
    expect(buildInput(tool("merge"), ["/a.pdf"], {})).toEqual({ inputs: ["/a.pdf"], opts: { blank_before_even: false } });
  });

  it("puts options at the top level otherwise", () => {
    expect(buildInput(tool("rotate"), ["/a.pdf", "/b.pdf"], { pages: "1-2" }))
      .toEqual({ input: "/a.pdf", pages: [0, 1], degrees: 90 });
  });

  it("nests dotted names, skips blank optional fields, converts colors", () => {
    expect(buildInput(tool("header-footer"), ["/a.pdf"], { "header.left": "H %[page]", "footer.right": "F" })).toEqual({
      input: "/a.pdf",
      opts: { header: { left: "H %[page]" }, footer: { right: "F" } },
    });
    expect(buildInput(tool("watermark"), ["/a.pdf"], { text: "DRAFT", color: "#ff8000", pages: "2" })).toEqual({
      input: "/a.pdf",
      opts: { text: "DRAFT", color: [255, 128, 0], pages: [1] },
    });
    expect(buildInput(tool("bates"), ["/a.pdf"], { prefix: "X", digits: "4", position: "top_left", pages: " " })).toEqual({
      input: "/a.pdf",
      opts: { prefix: "X", digits: 4, position: "top_left" },
    });
  });

  it("still requires non-optional pages", () => {
    expect(() => buildInput(tool("extract"), ["/a.pdf"], {})).toThrow("Enter at least one page");
  });
});

describe("humanize", () => {
  it("turns snake_case into a label", () => {
    expect(humanize("bottom_center")).toBe("Bottom Center");
    expect(humanize("center")).toBe("Center");
  });
});

describe("op tools", () => {
  it("cover every op command of the backend", () => {
    const ops = ["pages_extract", "pages_delete", "pages_rotate", "pages_reorder", "pages_organize", "pages_merge",
      "pages_alternate_mix", "pages_split_pages", "pages_split_size", "pages_split_text", "pages_split_bookmarks",
      "pages_nup", "pages_crop", "pages_insert_blank", "pages_split_half", "pages_extract_images",
      "content_page_numbers", "content_header_footer", "content_watermark", "content_bates",
      "content_compress", "content_set_metadata", "content_flatten", "content_delete_annotations",
      "content_repair", "content_linearize", "security_encrypt", "security_decrypt", "security_unlock", "security_inspect",
      "security_redact", "annotate_save", "ocr_run", "forms_fill", "forms_sign_image", "forms_sign_text",
      "convert_images_to_pdf", "convert_pdf_to_images", "read_text", "convert_office_to_pdf"];
    expect(TOOLS.map((t) => t.command).sort()).toEqual(ops.sort());
    expect(new Set(TOOLS.map((t) => t.id)).size).toBe(TOOLS.length);
  });

  it("parses 1-based ranges into 0-based inclusive ranges", () => {
    expect(parseRanges("1-3, 5")).toEqual([{ start: 0, end: 2 }, { start: 4, end: 4 }]);
    expect(() => parseRanges("3-1")).toThrow("Invalid page range: 3-1");
    expect(() => parseRanges(" ")).toThrow("Enter at least one page");
  });

  it("builds split, alternate mix and crop inputs", () => {
    expect(buildInput(tool("split-pages"), ["/a.pdf"], { ranges: "1, 2-3", pattern: "" }))
      .toEqual({ input: "/a.pdf", ranges: [{ start: 0, end: 0 }, { start: 1, end: 2 }] });
    expect(() => buildInput(tool("split-pages"), ["/a.pdf"], {})).toThrow("Enter at least one page");
    expect(buildInput(tool("split-bookmarks"), ["/a.pdf"], { pattern: "%[bookmark].pdf" }))
      .toEqual({ input: "/a.pdf", level: 1, pattern: "%[bookmark].pdf" });
    expect(buildInput(tool("alternate-mix"), ["/a.pdf", "/b.pdf"], { reverse_b: true }))
      .toEqual({ a: "/a.pdf", b: "/b.pdf", reverse_b: true });
    expect(buildInput(tool("crop"), ["/a.pdf"], { pages: "1", "rect.left": "10", "rect.bottom": "20", "rect.right": "110", "rect.top": "120" }))
      .toEqual({ input: "/a.pdf", pages: [0], rect: { left: 10, bottom: 20, right: 110, top: 120 } });
    expect(buildInput(tool("extract-images"), ["/a.pdf"], {})).toEqual({ input: "/a.pdf" });
  });

  it("builds compress, metadata and document inputs", () => {
    expect(buildInput(tool("compress"), ["/a.pdf"], { dpi: "72", jpeg_quality: "" }))
      .toEqual({ input: "/a.pdf", opts: { dpi: 72, discard_media: false, strip_metadata: false } });
    expect(buildInput(tool("set-metadata"), ["/a.pdf"], { title: "T", author: " " }))
      .toEqual({ input: "/a.pdf", metadata: { title: "T" } });
    expect(buildInput(tool("delete-annotations"), ["/a.pdf"], { pages: "2" })).toEqual({ input: "/a.pdf", pages: [1] });
    expect(buildInput(tool("delete-annotations"), ["/a.pdf"], {})).toEqual({ input: "/a.pdf" });
    for (const id of ["flatten", "repair", "linearize"]) expect(buildInput(tool(id), ["/a.pdf"], {})).toEqual({ input: "/a.pdf" });
  });

  it("builds security inputs with passwords as given", () => {
    expect(buildInput(tool("encrypt"), ["/a.pdf"], { user_pw: " u ", owner_pw: "o", "permissions.print": "low", "permissions.extract": false }))
      .toEqual({
        input: "/a.pdf",
        opts: {
          user_pw: " u ", owner_pw: "o", aes256: true,
          permissions: { print: "low", modify: true, extract: false, annotate: true, fill_forms: true, assemble: true },
        },
      });
    expect(buildInput(tool("decrypt"), ["/a.pdf"], { password: "p" })).toEqual({ input: "/a.pdf", password: "p" });
    expect(buildInput(tool("unlock"), ["/a.pdf"], {})).toEqual({ input: "/a.pdf" });
    expect(buildInput(tool("inspect-security"), ["/a.pdf"], { password: "" })).toEqual({ input: "/a.pdf" });
  });
});

describe("redact", () => {
  const rect = { "rect.left": "10", "rect.bottom": "20", "rect.right": "110.5", "rect.top": "120" };
  it("sends one box per chosen page, all with the same user-space rect", () => {
    expect(buildInput(tool("redact"), ["/a.pdf"], { pages: "1, 3", ...rect }, ["pw"])).toEqual({
      input: "/a.pdf", password: "pw",
      boxes: [0, 2].map((page) => ({ page, rect: { left: 10, bottom: 20, right: 110.5, top: 120 } })),
    });
    expect(() => buildInput(tool("redact"), ["/a.pdf"], rect)).toThrow("Enter at least one page");
  });

  it("warns before applying that redacted text is no longer searchable", () => {
    expect(tool("redact").hint).toMatch(/no longer searchable/);
  });
});

describe("sign", () => {
  const place = {
    "placement.page": "2", "placement.rect.left": "10", "placement.rect.bottom": "20",
    "placement.rect.right": "110", "placement.rect.top": "60",
  };
  const placement = { page: 1, rect: { left: 10, bottom: 20, right: 110, top: 60 }, rotate: 0, opacity: 1 };
  it("sends one 0-based page, the user-space rect, turn and opacity", () => {
    expect(buildInput(tool("sign-image"), ["/a.pdf"], { image: "/s.png", ...place }, ["pw"]))
      .toEqual({ input: "/a.pdf", password: "pw", image: "/s.png", placement });
    expect(buildInput(tool("sign-text"), ["/a.pdf"], { text: "Ann", ...place, "placement.rotate": "90" }))
      .toEqual({ input: "/a.pdf", text: "Ann", font_size: 24, placement: { ...placement, rotate: 90 } });
  });

  it("refuses more or less than one page", () => {
    expect(() => buildInput(tool("sign-text"), ["/a.pdf"], { ...place, "placement.page": "1-2" })).toThrow("Enter one page");
    expect(() => buildInput(tool("sign-text"), ["/a.pdf"], { ...place, "placement.page": "" })).toThrow("Enter at least one page");
  });

  it("says it is not a digital signature", () => {
    for (const id of ["sign-image", "sign-text"])
      expect(tool(id).hint).toContain("this places a signature image or text; it is not a digital (cryptographic) signature");
  });
});

describe("ocr", () => {
  it("sends the chosen language as a list, or none for the core default", () => {
    expect(buildInput(tool("ocr"), ["/a.pdf"], { lang: "deu" })).toEqual({ input: "/a.pdf", lang: ["deu"], dpi: 300 });
    expect(buildInput(tool("ocr"), ["/a.pdf"], { dpi: "150" }, ["pw"])).toEqual({ input: "/a.pdf", password: "pw", dpi: 150 });
    expect(tool("ocr").hint).toMatch(/links, form fields, annotations and bookmarks are not kept/);
  });
});

describe("convert", () => {
  it("sends the paper size in points, turned for landscape, or none for the image size", () => {
    const t = tool("images-to-pdf");
    expect(buildInput(t, ["/a.png", "/b.jpg"], {})).toEqual({ inputs: ["/a.png", "/b.jpg"] });
    expect(buildInput(t, ["/a.png"], { paper: "a4" }))
      .toEqual({ inputs: ["/a.png"], page_size: { width: 595.28, height: 841.89 } });
    expect(buildInput(t, ["/a.png"], { paper: "letter", orientation: "landscape" }))
      .toEqual({ inputs: ["/a.png"], page_size: { width: 792, height: 612 } });
    expect(t.accepts).toEqual([".png", ".jpg", ".jpeg"]);
  });

  it("builds pdf to images and pdf to text inputs", () => {
    expect(buildInput(tool("pdf-to-images"), ["/a.pdf"], { format: "jpeg", pages: "2" }))
      .toEqual({ input: "/a.pdf", dpi: 150, format: "jpeg", jpeg_quality: 85, pages: [1] });
    expect(buildInput(tool("pdf-to-text"), ["/a.pdf"], {})).toEqual({ input: "/a.pdf" });
    expect(tool("pdf-to-text")).toMatchObject({ title: "PDF to Text", group: "Convert" });
  });

  it("takes the dropped files a tool accepts, case-insensitively", () => {
    const files = ["/a/x.PNG", "/a/y.pdf", "/a/z.jpeg", "/a/t.txt"];
    expect(acceptedFiles(tool("images-to-pdf"), files)).toEqual(["/a/x.PNG", "/a/z.jpeg"]);
    expect(acceptedFiles(tool("rotate"), files)).toEqual(["/a/y.pdf"]);
  });
});

describe("input passwords", () => {
  it("go at the top level of a single-file tool, never under optsKey", () => {
    expect(buildInput(tool("compress"), ["/a.pdf"], {}, ["p w"]))
      .toEqual({ input: "/a.pdf", password: "p w", opts: { discard_media: false, strip_metadata: false } });
    expect(buildInput(tool("rotate"), ["/a.pdf"], { pages: "1" }, [" "]))
      .toEqual({ input: "/a.pdf", pages: [0], degrees: 90, password: " " });
    expect(buildInput(tool("split-half"), ["/a.pdf"], {}, [""])).toEqual({ input: "/a.pdf" });
  });

  it("go one per file for merge and alternate mix, blank as null", () => {
    expect(buildInput(tool("merge"), ["/a.pdf", "/b.pdf"], {}, ["", "b"]))
      .toEqual({ inputs: ["/a.pdf", "/b.pdf"], opts: { blank_before_even: false }, passwords: [null, "b"] });
    expect(buildInput(tool("alternate-mix"), ["/a.pdf", "/b.pdf"], {}, ["a", ""]))
      .toEqual({ a: "/a.pdf", b: "/b.pdf", reverse_b: false, passwords: ["a", null] });
    expect(buildInput(tool("merge"), ["/a.pdf"], {}, [""])).toEqual({ inputs: ["/a.pdf"], opts: { blank_before_even: false } });
  });

  it("are asked for by every tool but those taking a password themselves", () => {
    const own = TOOLS.filter((t) => t.ownPassword).map((t) => t.id).sort();
    expect(own).toEqual(["decrypt", "inspect-security", "unlock"]);
    expect(TOOLS.some((t) => t.options.some((f) => f.name === "password" && !t.ownPassword))).toBe(false);
  });
});
