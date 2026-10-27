---
created_on: 2026-10-14 06:09
last_modified: 2026-10-14 06:09
status: current
---

# Render fixture provenance

Fixtures derive from the MIT-licensed [go-remarkable-render](https://github.com/alexgorbatchev/go-remarkable-render) source at revision `61287a92812e50f9778bac2775a77ba2ca882aaa`.

- `linked-pages.pdf` is upstream `testdata/linked_pages.pdf`.
- `oct1-notes-template.pdf`, `oct1-notes-strokes.rm`, `oct1-notes-golden.png` are upstream `testdata/oct1_notes_template.pdf`, `oct1_notes_strokes.rm`, `oct1_notes_golden.png`.
- `links.pdf` and `links-base.pdf` use upstream `buildLinkPDF` with absent/present URI bases. They contain internal destinations, raw URI actions, unsupported actions, reversed and missing rectangles, bounded text and an astral character.
- `planner.pdf` uses upstream `buildMinimalPDF` with headers covering ordinary dates, missing fields, duplicate views, malformed day numbers and impossible Gregorian dates.
- `sample.rm` uses upstream `buildSampleRMStrokes`: ballpoint ink and a yellow highlighter.
- `background-go.png` and `overlay-go.png` are upstream `RenderPlannerPage` outputs for `planner.pdf` at 72 DPI, without/with `sample.rm`.
- `oct1-notes-go.png` is upstream `RenderPlannerPage` output for the original planner PDF and native strokes at 200 DPI.

Synthetic fixtures and Go baselines are reproducible using the original helper functions and renderer; the local generation program and execution evidence live under `.tmp/oracle/`. Upstream source licensing is retained in the root `LICENSE`.
