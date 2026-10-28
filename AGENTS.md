---
created_on: 2026-10-14 06:09
last_modified: 2026-10-14 06:09
status: current
---

# remarkable-render

Native PDF inspection and reMarkable stroke previews. Inherit the parent `AGENTS.md`.

## Commands

- Build: `cargo build --all-targets`.
- Test including README example: `cargo test`.
- Format: `cargo fmt --package remarkable-render -- --check`.
- Lint: `cargo clippy --all-targets -- -D warnings`.
- Coverage: `cargo llvm-cov --all-targets --fail-under-lines 90`.
- Full check: `just check`.
- API documentation: `cargo doc --no-deps`.

## Native ownership

- `src/native.rs` owns the process-wide PDFium runtime. Keep initialization once-only, the dynamic bindings alive for the process, and every native operation, including destruction, under the engine mutex. Do not construct a separate high-level `Pdfium` owner, which independently initializes/destroys the same global runtime.
- Native document bytes must outlive `FPDF_DOCUMENT`; text pages close before pages, and pages/bitmaps close before releasing their lock. Keep safety arguments beside every unsafe operation and Send/Sync implementation.
- Bind public `PdfiumLibraryBindings` directly. High-level URI helpers reject invalid UTF-8 and cannot preserve this library's raw URI contract. Verify library bytes against `pdfium-bundled`'s platform pin even for an explicit `PDFIUM_LIB_PATH`; do not bypass verification.
- Keep `pdfium-render` and `pdfium-bundled` API/build versions compatible. `pdfium_latest` currently binds PDFium 8066 through the pinned dependency. Inspect upstream API and binary requirements before changing either dependency.

## Behavior and verification

- Preserve action precedence: an existing action suppresses a direct destination, even when unsupported or unresolved. Keep PDFium's URI base concatenation and all raw bytes, removing only the terminal NUL.
- Bounds are normalized default-user-space PDF coordinates, independent of crop/rotation. Bounded text uses PDFium's UTF-16 length query, including surrogate pairs, and collapses Unicode whitespace.
- The renderer uses reversed native bitmap byte order for RGBA and source-over stroke compositing. Preserve white versus transparent background selection using `FPDFPage_HasTransparency`, DPI ceiling and bitmap size checks. Do not enable annotation rendering implicitly.
- Planner date validity follows the supplied Gregorian year. Weekday names classify a header but need not match the calculated weekday. `Notes` takes precedence over `Day`; later duplicate pages win.
- Tests in `tests/render.rs` exercise real PDFium, native strokes, original fixtures and Go-generated raster baselines. Preserve meaningful tolerances and source provenance in `tests/fixtures/PROVENANCE.md`; do not replace native tests with fake bindings.
- Runtime-result changes require corresponding test-file changes and at least 90% line coverage, excluding `scripts/`. Record red/green and implementation-disable proof under `.tmp/`.
- Format only this package. External libraries resolve from pinned GitHub revisions.

## Boundaries

- Keep source snapshots and sibling projects unchanged. Temporary artifacts belong in `.tmp/`; worktrees belong in `.workspaces/`.
- Persist standing guidance here; keep task progress, handoffs and check results outside agent instructions.
- Never publish releases, tags, packages or deployments without explicit authorization.
