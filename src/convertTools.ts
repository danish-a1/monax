// The Convert group's entries after Extract Images (split from tools.ts for its size cap).
import { pageSize } from "./lib/input";
import type { FieldSpec, ToolEntry } from "./tools";

/** Why Office to PDF is greyed out (R152). */
export const OFFICE_MISSING = "WordCraft not found - the bundled converter is missing.";

const allPages: FieldSpec = { name: "pages", label: "Pages", kind: "pages", optional: true };
const OFFICE = ["docx", "doc", "odt", "rtf", "txt", "html", "htm", "md"].map((e) => `.${e}`);

export const CONVERT_TOOLS: ToolEntry[] = [
  {
    id: "images-to-pdf", title: "Images to PDF", group: "Convert", command: "convert_images_to_pdf", multi: true,
    accepts: [".png", ".jpg", ".jpeg"], shape: pageSize, options: [
      { name: "paper", label: "Page Size", kind: "select", choices: ["same_as_image", "a4", "letter"], default: "same_as_image" },
      { name: "orientation", label: "Orientation", kind: "select", choices: ["portrait", "landscape"], default: "portrait" }],
  },
  {
    id: "pdf-to-images", title: "PDF to Images", group: "Convert", command: "convert_pdf_to_images", options: [
      { name: "dpi", label: "Resolution (DPI)", kind: "number", default: 150 },
      { name: "format", label: "Format", kind: "select", choices: ["png", "jpeg"] },
      { name: "jpeg_quality", label: "JPEG Quality", kind: "number", default: 85 }, allPages],
  },
  { id: "pdf-to-text", title: "PDF to Text", group: "Convert", command: "read_text", options: [allPages] },
  {
    id: "office-to-pdf", title: "Office to PDF", group: "Convert", command: "convert_office_to_pdf", requires: "office",
    accepts: OFFICE, options: [],
  },
];
