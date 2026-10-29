import { CONVERT_TOOLS } from "./convertTools";
import { onePage } from "./lib/input";

export type Group = "Organize" | "Convert" | "Security" | "Edit" | "Sign" | "OCR";
export const GROUPS: Group[] = ["Organize", "Convert", "Security", "Edit", "Sign", "OCR"];

export interface FieldSpec {
  /** Key in the command input; dots nest ("header.left"). */
  name: string;
  label: string;
  /** pages: "1-3, 5" -> 0-based list; ranges: "1-3, 5" -> 0-based {start, end} list. */
  kind: "pages" | "ranges" | "number" | "checkbox" | "text" | "select" | "color" | "password";
  /** Without one, a blank field is left out and the core default applies. */
  default?: string | number | boolean;
  /** pages: blank means every page. */
  optional?: boolean;
  /** select: allowed values (snake_case, shown humanized). */
  choices?: string[];
  /** select: command returning the allowed values (installed OCR languages). */
  choicesCommand?: string;
}

export interface ToolEntry {
  id: string;
  title: string;
  group: Group;
  command: string;
  options: FieldSpec[];
  /** Input files the tool needs: one PDF, or many (merge). */
  multi?: boolean;
  /** Send the dropped files under these keys, in order (alternate mix: a, b), instead of `input`. */
  fileKeys?: string[];
  /** Custom panel instead of the generic field form. */
  custom?: boolean;
  /** Nest the option fields under this key (merge: `opts`) instead of the top level. */
  optsKey?: string;
  /** Takes a password itself (decrypt, inspect) or none (unlock): never asked for an input password. */
  ownPassword?: boolean;
  /** Shown above the fields: what the user must know before applying. */
  hint?: string;
  /** Reshapes the built input (redact: chosen pages x one rect -> boxes). */
  shape?: (input: Record<string, unknown>) => object;
  /** Dropped file extensions the tool takes (lowercase); default [".pdf"]. */
  accepts?: string[];
  /** Needs the bundled WordCraft: greyed out without it. */
  requires?: "office";
}

const pages: FieldSpec = { name: "pages", label: "Pages", kind: "pages" };
const allPages: FieldSpec = { ...pages, optional: true };
const text = (name: string, label: string): FieldSpec => ({ name, label, kind: "text" });
const num = (name: string, label: string): FieldSpec => ({ name, label, kind: "number" });
const ANCHORS = ["top_left", "top_center", "top_right", "middle_left", "center", "middle_right", "bottom_left", "bottom_center", "bottom_right"];
const position: FieldSpec = { name: "position", label: "Position", kind: "select", choices: ANCHORS };
const rows = ["header", "footer"].flatMap((r) =>
  ["left", "center", "right"].map((c) => text(`${r}.${c}`, humanize(`${r}_${c}`))),
);

const pattern: FieldSpec = text("pattern", "File Name Pattern");
const rect = ["left", "bottom", "right", "top"].map((k) => num(`rect.${k}`, `${humanize(k)} (pt)`));

// Signature placement: one page, a user-space rect, a clockwise turn as displayed, opacity.
const placement: FieldSpec[] = [
  { name: "placement.page", label: "Page", kind: "pages" },
  ...["left", "bottom", "right", "top"].map((k) => num(`placement.rect.${k}`, `${humanize(k)} (pt)`)),
  { name: "placement.rotate", label: "Rotate (Clockwise: 0, 90, 180, 270)", kind: "number", default: 0 },
  { name: "placement.opacity", label: "Opacity", kind: "number", default: 1 },
];
const signHint = "Note: this places a signature image or text; it is not a digital (cryptographic) signature.";

const secret = (name: string, label: string): FieldSpec => ({ name, label, kind: "password" });
const permissions: FieldSpec[] = [
  { name: "permissions.print", label: "Printing", kind: "select", choices: ["none", "low", "high"] },
  ...["modify", "extract", "annotate", "fill_forms", "assemble"].map((k): FieldSpec => (
    { name: `permissions.${k}`, label: `Allow ${humanize(k)}`, kind: "checkbox", default: true })),
];

// Only tools whose Tauri commands exist. Hand-written: field names match the core input structs.
export const TOOLS: ToolEntry[] = [
  { id: "organize", title: "Organize", group: "Organize", command: "pages_organize", options: [], custom: true },
  { id: "extract", title: "Extract Pages", group: "Organize", command: "pages_extract", options: [pages] },
  { id: "delete", title: "Delete Pages", group: "Organize", command: "pages_delete", options: [pages] },
  {
    id: "rotate", title: "Rotate Pages", group: "Organize", command: "pages_rotate",
    options: [pages, { name: "degrees", label: "Degrees", kind: "number", default: 90 }],
  },
  { id: "reorder", title: "Reorder Pages", group: "Organize", command: "pages_reorder", options: [{ name: "order", label: "Order", kind: "pages" }] },
  {
    id: "merge", title: "Merge", group: "Organize", command: "pages_merge", multi: true, optsKey: "opts",
    options: [{ name: "blank_before_even", label: "Blank Before Even", kind: "checkbox", default: false }],
  },
  {
    id: "alternate-mix", title: "Alternate Mix", group: "Organize", command: "pages_alternate_mix", multi: true, fileKeys: ["a", "b"],
    options: [{ name: "reverse_b", label: "Reverse Second File", kind: "checkbox", default: false }],
  },
  { id: "split-pages", title: "Split by Ranges", group: "Organize", command: "pages_split_pages", options: [{ name: "ranges", label: "Ranges", kind: "ranges" }, pattern] },
  { id: "split-size", title: "Split by Size", group: "Organize", command: "pages_split_size", options: [num("max_bytes", "Max Bytes"), pattern] },
  { id: "split-text", title: "Split by Text", group: "Organize", command: "pages_split_text", options: [text("text", "Text"), pattern] },
  {
    id: "split-bookmarks", title: "Split by Bookmarks", group: "Organize", command: "pages_split_bookmarks",
    options: [{ name: "level", label: "Level", kind: "number", default: 1 }, pattern],
  },
  { id: "split-half", title: "Split Half", group: "Organize", command: "pages_split_half", options: [] },
  {
    id: "nup", title: "N-up", group: "Organize", command: "pages_nup",
    options: [{ name: "per_sheet", label: "Pages Per Sheet", kind: "number", default: 4 }],
  },
  { id: "insert-blank", title: "Insert Blank Pages", group: "Organize", command: "pages_insert_blank", options: [{ name: "after", label: "After Pages", kind: "pages" }] },
  { id: "viewer", title: "View & Annotate", group: "Edit", command: "annotate_save", options: [], custom: true },
  { id: "crop", title: "Crop", group: "Edit", command: "pages_crop", options: [pages, ...rect] },
  { id: "extract-images", title: "Extract Images", group: "Convert", command: "pages_extract_images", options: [] },
  ...CONVERT_TOOLS,
  {
    id: "page-numbers", title: "Page Numbers", group: "Edit", command: "content_page_numbers", optsKey: "opts",
    options: [text("format", "Format"), position, num("margin", "Margin"), num("size", "Size"), num("start_at", "Start At"), allPages],
  },
  {
    id: "header-footer", title: "Header & Footer", group: "Edit", command: "content_header_footer", optsKey: "opts",
    options: [...rows, num("size", "Size"), num("margin", "Margin"), allPages],
  },
  {
    id: "watermark", title: "Watermark", group: "Edit", command: "content_watermark", optsKey: "opts",
    options: [
      text("text", "Text"), num("opacity", "Opacity"), num("angle", "Angle"), num("size", "Size"),
      { name: "color", label: "Color", kind: "color" }, position, num("margin", "Margin"), allPages,
    ],
  },
  {
    id: "bates", title: "Bates Numbering", group: "Edit", command: "content_bates", optsKey: "opts",
    options: [text("prefix", "Prefix"), text("suffix", "Suffix"), num("start", "Start"), num("digits", "Digits"), position, num("margin", "Margin"), num("size", "Size"), allPages],
  },
  {
    id: "compress", title: "Compress", group: "Edit", command: "content_compress", optsKey: "opts",
    options: [
      num("dpi", "Image DPI"), num("jpeg_quality", "JPEG Quality"),
      { name: "discard_media", label: "Discard Media", kind: "checkbox", default: false },
      { name: "strip_metadata", label: "Strip Metadata", kind: "checkbox", default: false },
    ],
  },
  {
    id: "set-metadata", title: "Edit Metadata", group: "Edit", command: "content_set_metadata", optsKey: "metadata",
    options: ["title", "author", "subject", "keywords", "creator", "producer"].map((k) => text(k, humanize(k))),
  },
  { id: "flatten", title: "Flatten", group: "Edit", command: "content_flatten", options: [] },
  { id: "delete-annotations", title: "Delete Annotations", group: "Edit", command: "content_delete_annotations", options: [allPages] },
  { id: "repair", title: "Repair", group: "Edit", command: "content_repair", options: [] },
  { id: "linearize", title: "Linearize (Web Optimize)", group: "Edit", command: "content_linearize", options: [] },
  {
    id: "encrypt", title: "Encrypt", group: "Security", command: "security_encrypt", optsKey: "opts",
    options: [secret("user_pw", "User Password"), secret("owner_pw", "Owner Password"), ...permissions,
      { name: "aes256", label: "AES-256 (Off: AES-128)", kind: "checkbox", default: true }],
  },
  {
    id: "decrypt", title: "Remove Password", group: "Security", command: "security_decrypt", ownPassword: true,
    options: [secret("password", "Password")],
  },
  { id: "unlock", title: "Remove Restrictions", group: "Security", command: "security_unlock", options: [], ownPassword: true },
  {
    id: "inspect-security", title: "Security Info", group: "Security", command: "security_inspect", ownPassword: true,
    options: [secret("password", "Password")],
  },
  {
    id: "redact", title: "Redact", group: "Security", command: "security_redact",
    hint: "Redacted pages become images: text on them is no longer searchable.",
    options: [pages, ...rect],
    shape: ({ pages, rect, ...rest }) => ({ ...rest, boxes: (pages as number[]).map((page) => ({ page, rect })) }),
  },
  { id: "fill-form", title: "Fill Form", group: "Sign", command: "forms_fill", options: [], custom: true },
  {
    id: "sign-image", title: "Sign with Image", group: "Sign", command: "forms_sign_image", hint: signHint, shape: onePage,
    options: [text("image", "Signature Image (PNG or JPEG Path)"), ...placement],
  },
  {
    id: "sign-text", title: "Sign with Text", group: "Sign", command: "forms_sign_text", hint: signHint, shape: onePage,
    options: [text("text", "Signature Text"), { name: "font_size", label: "Font Size", kind: "number", default: 24 }, ...placement],
  },
  {
    id: "ocr", title: "OCR (Searchable PDF)", group: "OCR", command: "ocr_run",
    hint: "Pages become images with a searchable text layer: links, form fields, annotations and bookmarks are not kept.",
    options: [
      { name: "lang", label: "Language", kind: "select", choicesCommand: "ocr_languages" },
      { name: "dpi", label: "Resolution (DPI)", kind: "number", default: 300 },
    ],
    shape: ({ lang, ...rest }) => (lang === undefined ? rest : { ...rest, lang: [lang] }),
  },
];

/** "bottom_center" -> "Bottom Center". */
export function humanize(id: string): string {
  return id.split("_").map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join(" ");
}

export { acceptedFiles, buildInput, parsePages, parseRanges, type Values } from "./lib/input";
