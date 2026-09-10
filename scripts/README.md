# Scripts

`check-spec-citations.py` verifies that `WHITEPAPER.md §N.N` (and ranges) in
`docs/specification/` and Rust source cite headings that exist in `WHITEPAPER.md`.
Run locally with `python3 scripts/check-spec-citations.py`.

`fix_whitepaper_page_headers.py` finds PDF page headers/footers (a bare page
number followed by the running chapter/section title) that got interleaved
into `WHITEPAPER.md`'s flowing text during the original PDF-to-Markdown
conversion. It removes the ones that fall cleanly on a paragraph/list-item
boundary and reports the rest (mid-sentence or mid-word breaks) to
`docs/whitepaper-page-header-review.md` for manual reconstruction, since
restoring the original wording there requires human judgment. Run with
`python3 scripts/fix_whitepaper_page_headers.py [--apply] [--report PATH]`
(omit `--apply` for a dry run).
