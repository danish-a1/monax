# Monax

Monax is a local, offline PDF editor for macOS with the tool set of Sejda PDF Desktop:
merge, split, organize, compress, watermark, encrypt, redact, OCR, forms, sign, convert
and annotate. There is no account, no license server, no subscription and no network
access at runtime. An embedded MCP server lets an agent such as Claude Code drive every
operation against the running app. MIT licensed, built from source.

## Features

- **Pages:** merge, alternate mix, split (by pages, size, text, bookmarks, half page),
  extract, delete, rotate, reorder, organize, insert blank, N-up, crop.
- **Content:** page numbers, header and footer, watermark, Bates numbering, extract
  images, read text, metadata, compress, flatten, delete annotations, repair, linearize.
- **Security:** encrypt, decrypt, unlock, inspect security, and true redaction (the page
  is rasterized at 300 dpi and flattened, the original content stream is dropped).
- **OCR:** searchable text layer via tesseract.
- **Forms and sign:** list and fill form fields, sign with an image or typed text.
- **Convert:** images to PDF, PDF to images, Office to PDF via WordCraft (docx, doc, odt, rtf, txt, html, md).
- **Annotate:** add annotations in PDF user space.
- **Viewer:** PDF.js viewer with thumbnails.
- **MCP:** every operation as an MCP tool over local streamable HTTP.

Every operation writes a new file and never modifies its input; a taken output name gets
a `-1`, `-2` suffix.

## Requirements

- macOS 11 or later (`tauri.conf.json` sets no `minimumSystemVersion`; Rust's Apple
  silicon target needs 11). Tested on macOS 27, Apple silicon.
- Apple silicon or Intel.
- Xcode Command Line Tools, Homebrew, Rust 1.95 or later (rustup), Node.js 20 or later.

`scripts/setup.sh` checks all four and prints the install command for any that is missing.

## Quick start

```bash
git clone <repo-url> monax && cd monax && scripts/setup.sh && scripts/run.sh
```

`setup.sh` installs qpdf and tesseract with Homebrew if absent, downloads pdfium
(bblanchon/pdfium-binaries `chromium/8125`, checksum verified) into `vendor/pdfium`, runs
`npm ci`, then `scripts/fetch-sidecars.sh`, which copies qpdf and tesseract into
`sidecars/` and builds WordCraft from source at a pinned revision. The first setup and
build take several minutes; later starts are instant. `scripts/setup.sh --check` only
reports what is present.

## Running

| Command | What it does |
|---|---|
| `scripts/run.sh` | Builds the release binary once (`npm run tauri build -- --no-bundle`), then runs `target/release/monax`. |
| `scripts/run.sh --dev` | Runs `npm run tauri dev`: Vite dev server with hot reload plus a debug build. |

Both set `MONAX_BIN_DIR=sidecars` and `MONAX_LIBPDFIUM=vendor/pdfium/lib/libpdfium.dylib`
and stop with a clear message if a sidecar or pdfium is missing. `run.sh` does not
rebuild after a `git pull`; run `npm run tauri build -- --no-bundle` to rebuild.

Running from source never needs a DMG or `.app`. The binary is built on your machine, so
it is not notarized and Gatekeeper does not get involved.

## Using with MCP agents

Monax must be running: the MCP server lives inside the app.

- **Endpoint:** `http://127.0.0.1:<port>/mcp` (streamable HTTP). It binds `127.0.0.1`
  only and every request needs the bearer token.
- **URL and token:** in the app, Settings > MCP Settings shows both plus a copyable
  `claude mcp add` command. They are also in
  `~/Library/Application Support/monax/mcp.json` (`url`, `token`).

Claude Code:

```bash
claude mcp add --transport http monax http://127.0.0.1:<port>/mcp --header "Authorization: Bearer <token>"
```

Other MCP clients that speak streamable HTTP:

```json
{
  "mcpServers": {
    "monax": {
      "url": "http://127.0.0.1:<port>/mcp",
      "headers": { "Authorization": "Bearer <token>" }
    }
  }
}
```

Clients that only speak stdio need an HTTP bridge.

**Security:** the token is a secret; do not commit it or paste it into shared configs.
Monax reuses its port and token across launches. If the port is taken, it picks a new
port and a new token (`port_changed`, shown in MCP Settings); re-run the connection
command then.

**Jobs:** every tool returns a `job_id` at once; poll `pdf_job_status` until the job is
done, then read `outputs`. `pdf_job_cancel` cancels. `pdf_capabilities` reports the qpdf,
tesseract and WordCraft versions and the OCR languages available.

| Group | Tools |
|---|---|
| Jobs and app | `pdf_capabilities`, `pdf_job_status`, `pdf_job_cancel`, `pdf_open_in_app` |
| Read | `pdf_page_info`, `pdf_read_text`, `pdf_render_page`, `pdf_inspect_security`, `pdf_list_fields` |
| Pages | `pdf_extract`, `pdf_delete`, `pdf_rotate`, `pdf_reorder`, `pdf_organize`, `pdf_merge`, `pdf_alternate_mix`, `pdf_split_pages`, `pdf_split_size`, `pdf_split_text`, `pdf_split_bookmarks`, `pdf_split_half`, `pdf_nup`, `pdf_crop`, `pdf_insert_blank` |
| Content | `pdf_extract_images`, `pdf_page_numbers`, `pdf_header_footer`, `pdf_watermark`, `pdf_bates`, `pdf_compress`, `pdf_set_metadata`, `pdf_flatten`, `pdf_delete_annotations`, `pdf_repair`, `pdf_linearize` |
| Security | `pdf_encrypt`, `pdf_decrypt`, `pdf_unlock`, `pdf_redact` |
| OCR, forms, sign, annotate | `pdf_ocr`, `pdf_fill_forms`, `pdf_sign_image`, `pdf_sign_text`, `pdf_annotate` |
| Convert | `pdf_images_to_pdf`, `pdf_to_images`, `pdf_office_to_pdf` |

Paths are absolute. Coordinates are PDF user space: points, origin bottom-left, y-up,
relative to the unrotated MediaBox.

Example prompts:

- "Merge ~/Documents/q1.pdf and ~/Documents/q2.pdf with Monax, then add page numbers at
  the bottom center."
- "OCR ~/Scans/contract.pdf with Monax, then redact every occurrence of the client's
  account number."

## Building and testing

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
MONAX_LIBPDFIUM="$PWD/vendor/pdfium/lib/libpdfium.dylib" cargo test --workspace
npm run lint && npm test && npx tsc --noEmit
```

Run `cargo test` with `MONAX_BIN_DIR` unset: the OCR sidecar test sets up its own
tool layout and fails when the variable points at `sidecars/`. Tests whose sidecar or
fixture is absent skip with a message.

Environment variables:

| Variable | Purpose |
|---|---|
| `MONAX_BIN_DIR` | Directory holding `qpdf`, `tesseract`, `wordcraft-cli` (normally `sidecars/`). |
| `MONAX_LIBPDFIUM` | Absolute path of `libpdfium.dylib`. |
| `MONAX_CONFIG_DIR` | Absolute directory for `mcp.json` instead of `~/Library/Application Support/monax` (tests use a temp dir). |

## Project layout

```
crates/monax-core/     every PDF operation (pdfium, qpdf); no UI code
crates/monax-sidecar/  qpdf / tesseract / wordcraft-cli discovery and process runner
src-tauri/             Tauri shell: GUI commands and the embedded MCP server
src/                   React + TypeScript frontend (tools, panels, PDF.js viewer)
scripts/               setup.sh, run.sh, fetch-sidecars.sh, third-party.sh, smoke.sh
```

## Limitations

- No PDF to Office conversion.
- Office input is text documents only (docx, doc, odt, rtf, txt, html, md): no xlsx or pptx.
- Encrypted Office files are refused.
- No cryptographic (PAdES) signing; signatures are visual.
- macOS only.
- The viewer softens above about 175% zoom on retina displays.

## Third-party software

| Component | Use | License |
|---|---|---|
| [qpdf](https://github.com/qpdf/qpdf) | page surgery, encryption, repair | Apache-2.0 |
| [tesseract](https://github.com/tesseract-ocr/tesseract) | OCR | Apache-2.0 |
| [pdfium](https://pdfium.googlesource.com/pdfium/) ([binaries](https://github.com/bblanchon/pdfium-binaries)) | rendering and extraction | BSD-3-Clause and others |
| [WordCraft](https://github.com/storytold/wordcraft) (`wordcraft-cli`) | Office to PDF | MIT OR Apache-2.0 |

qpdf and tesseract are copied from Homebrew with their dylibs, each with its license, into
`sidecars/licenses/`. `scripts/third-party.sh` (run by `fetch-sidecars.sh`) generates
`sidecars/licenses/THIRD-PARTY.txt` for every Rust crate and npm package compiled into the
app and fails on a GPL, AGPL or LGPL-only license.

## License

MIT. See [LICENSE](LICENSE).
