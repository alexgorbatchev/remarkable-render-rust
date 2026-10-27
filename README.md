`remarkable-render` inspects PDFs and composites native reMarkable handwriting for Rust applications that browse or preview tablet documents.

# What It Does

- **PDF inspection**: Opens bytes, paths or readers and extracts page text and link annotations.
- **Raw link data**: Preserves URI bytes, internal page destinations, PDF rectangles and text inside each link.
- **Planner indexing**: Maps dated day and notes views to their zero-based page indexes.
- **Page previews**: Returns RGBA PNGs combining a PDF background and optional version 6 `.rm` strokes.

# How It Works

1. Open a PDF as a `Document`, or pass PDF bytes, a path or a reader directly to an operation.
2. Read page text and links, or index dated planner headers for a supplied year.
3. Render a selected page at the desired DPI, optionally supplying that page's native strokes.

# How it Really Works

- Calls block until their work finishes. Documents own their PDF bytes until they are dropped. A borrowed document stays open after indexing or rendering. Calls across all documents serialize; documents can be shared between threads, but PDF operations do not run in parallel.
- The first PDF operation loads a pinned native PDFium 8066 library. If absent, it downloads the platform archive from [pdfium-binaries](https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium%2F8066), checks its SHA-256 and caches the extracted library. The default cache is `$XDG_CACHE_HOME/pdfium-bundled/pdfium-8066/`, falling back to `~/.cache/pdfium-bundled/pdfium-8066/`. PDFium remains loaded for the process lifetime. Use this package as the process's sole PDFium runtime owner.
- File and reader inputs are read completely into memory. Rendering holds the background, stroke layer and encoded PNG in memory. It returns bytes without writing an image file. PDF content and strokes are not sent to a remote service.
- Page indexes start at zero. Link rectangles use default PDF user-space coordinates, with y increasing upwards; crop-box origins and page rotation are not applied. URI actions remain raw `Vec<u8>` data, including invalid UTF-8. A catalog URI base is concatenated according to PDFium's rules, without URL normalization. A URI resembling `#page=2` remains external unless the annotation has an internal destination.
- Text comes from the PDF text layer. Link text includes characters overlapping the rectangle and collapses whitespace. Handwriting and images are not OCR inputs.
- Planner headers begin with an English abbreviated month and day number and contain a full English weekday. `Notes` identifies the day view; `Day` identifies the notes view. The supplied year determines calendar validity. The weekday label is not cross-checked against that date, and the last matching page wins. Invalid dates, unrecognized headers and unreadable pages are skipped.
- Rendering uses `ceil(points × DPI / 72)` dimensions. Non-positive DPI selects 200. PDF annotations are omitted; native strokes are a visual overlay, with highlighters beneath pen ink. Output is a preview, not an editable tablet document. Invalid inputs and initialization failures return typed errors.

# Prerequisites

- A supported native PDFium target: Linux or macOS on x86-64 or ARM64, or Windows on x86, x86-64 or ARM64. Native execution is verified on Linux x86-64.
- Network access for the initial library download, or an existing copy of the pinned library selected through the environment below.

# Installation

From a sibling Rust application's directory:

```sh
cargo add remarkable-render --path ../remarkable-render
```

# Quick Start

```rust
use remarkable_render::{Document, index_planner_dates, render_planner_page};

fn main() -> remarkable_render::Result<()> {
    let pdf = include_bytes!("tests/fixtures/planner.pdf").as_slice();
    let document = Document::from_bytes(pdf)?;
    assert_eq!(document.num_pages(), 19);
    assert!(document.text(0)?.contains("Jan 1"));
    let dates = index_planner_dates(&document, 2026)?;
    assert!(dates.contains_key("2026-01-01"));
    let png = render_planner_page(&document, 0, &[], 72)?;
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    Ok(())
}
```

The example uses the included fixture; supply your PDF bytes or use `Document::from_path` for a local document. Pass that page's `.rm` bytes instead of `&[]` to include handwriting.

# API

| Entry point | Result | Purpose |
| :--- | :--- | :--- |
| `Document::from_bytes`, `from_path`, `from_reader` | `Result<Document>` | Opens and owns a PDF. |
| `Document::num_pages()` | `usize` | Returns the page count. |
| `Document::text(page)` | `Result<String>` | Extracts the PDF text layer. |
| `Document::links(page)` | `Result<Vec<PageLink>>` | Enumerates links, their destinations, raw URIs, rectangles and labels. |
| `index_planner_dates(source, year)` | `Result<PlannerIndex>` | Maps date strings to `PlannerKind::Day` and `PlannerKind::Notes` page indexes. |
| `render_planner_page(source, page, strokes, dpi)` | `Result<Vec<u8>>` | Produces an RGBA PNG. |

`DocumentSource` accepts a borrowed document, byte slice, byte vector, path or string path through `From`. Use `DocumentSource::Reader(&mut reader)` for a reader. `Error` distinguishes I/O, native initialization, digest verification, PDF operations, page bounds, year, dimensions, strokes, SVG and PNG errors.

# Configuration

Set environment variables before the first PDF operation; the runtime retains its initial library selection.

| Variable | Default | Purpose |
| :--- | :--- | :--- |
| `PDFIUM_LIB_PATH` | Platform cache/download | Selects an existing library file. Its bytes must match the platform's pinned PDFium 8066 digest. A missing file falls back to cache/download. |
| `PDFIUM_BUNDLED_CACHE_DIR` | `$XDG_CACHE_HOME/pdfium-bundled` or `~/.cache/pdfium-bundled` | Selects the cache root; `pdfium-8066` is appended. |
| `PDFIUM_NO_AUTO_DOWNLOAD` | Unset | Presence disables downloads; an intact cache or explicit library remains usable. |

# Compatibility

The behavior derives from [go-remarkable-render](https://github.com/alexgorbatchev/go-remarkable-render). Rust ownership closes handles automatically, internal destinations use `Option<usize>`, and URI bytes avoid UTF-8 coercion. Planner indexing rejects February 30/31, April 31 and February 29 in non-leap years. Rasterizer and alpha quantization differences mean previews are not byte-identical across implementations. Stroke-format and SVG limitations follow the sibling [rmscene](../rmscene/README.md) package.

# License

[MIT](./LICENSE), copyright 2026 Alex Gorbatchev.
