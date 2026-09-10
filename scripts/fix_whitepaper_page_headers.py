#!/usr/bin/env python3
"""Find and (optionally) remove injected PDF page headers/footers in WHITEPAPER.md.

During the PDF-to-Markdown conversion, every page break left behind a
three-line "running header" block interleaved into the flowing text:

    <blank>
    <bare page number, 1-3 digits>
    <blank>
    <chapter/section running header, e.g. "2.4. Messages Between Shardchains">
    <blank>

This block sits between whatever content came right before the page break
(PREV) and whatever comes right after it (NEXT). Sometimes PREV/NEXT land on
a clean paragraph or list-item boundary, and the block can simply be deleted.
Other times the page broke mid-sentence or mid-word, and naively deleting the
block would leave the surrounding text broken; reconstructing the original
wording requires human judgment, so those cases are only reported, never
auto-resolved.

Usage:
    python3 scripts/fix_whitepaper_page_headers.py [--file PATH] [--apply] [--report PATH]

Without --apply, the script only analyzes and reports (dry run). With
--apply, paragraph-boundary blocks are removed in place; mid-sentence/
mid-word blocks are left untouched and still reported for manual review.
"""

import argparse
import re
import sys

HEADING_RE = re.compile(r"^(#{1,6})\s+(.*?)\s*$")
NUMBER_LINE_RE = re.compile(r"^\d{1,3}$")
CHAPTER_NUM_RE = re.compile(r"^(\d+)\s+(.*)$")
APPENDIX_RE = re.compile(r"^([A-Z])\s+(.*)$")
SECTION_NUM_RE = re.compile(r"^(\d+\.\d+)\s+(.*)$")

# A PREV line ending in one of these (optionally followed by a closing
# quote/paren or a closing <sup> tag) is considered a clean sentence/clause
# end -> safe paragraph boundary.
TERMINAL_RE = re.compile(r"[.!?:][\"'”’)]*(</sup>)?\s*$")

# A NEXT line starting a new structural element is also a safe boundary,
# regardless of how PREV ends (headings, bullets, blockquotes, tables and
# freshly-numbered sub-points never appear mid-sentence in this document).
# Every marker below must be followed by a capital letter or digit, per
# this document's own list/heading style -- that is what distinguishes a
# genuine new item (e.g. "- The public key needed...") from a page break
# landing inside a wrapped continuation line that happens to start with a
# lowercase/parenthetical fragment (e.g. "   - (cf. 2.5.14), this enables
# ..." is a continuation of the previous bullet, not a new one; likewise a
# stray "#### who keeps track of..." is a pre-existing mis-tagged-heading
# artifact, not a real section break).
STRUCTURAL_NEXT_RE = re.compile(r"^(#{1,6}\s+[A-Z0-9]|-\s+[A-Z0-9]|>\s+[A-Z0-9]|\|)|^\d+\.\d+\.\d+\.\s+[A-Z0-9]")

HYPHEN_BREAK_RE = re.compile(r"[a-zA-Z]-\s*$")


def derive_valid_headers(lines):
    """Map every possible injected running-header text to its source heading line."""
    valid = {}
    for i, l in enumerate(lines):
        m = HEADING_RE.match(l)
        if not m:
            continue
        level, text = len(m.group(1)), m.group(2).strip()
        if level == 2:
            cm = CHAPTER_NUM_RE.match(text)
            am = APPENDIX_RE.match(text)
            if cm:
                valid[f"Chapter {cm.group(1)}. {cm.group(2)}"] = i
            elif am:
                valid[f"Appendix {am.group(1)}. {am.group(2)}"] = i
            else:
                valid[text] = i
        elif level == 3:
            sm = SECTION_NUM_RE.match(text)
            if sm:
                valid[f"{sm.group(1)}. {sm.group(2)}"] = i
    return valid


def find_blocks(lines, valid_headers):
    """Return list of (start_idx) for each 5-line injected block: blank,num,blank,header,blank."""
    blocks = []
    i, n = 0, len(lines)
    while i < n - 4:
        if (
            lines[i].strip() == ""
            and NUMBER_LINE_RE.match(lines[i + 1].strip())
            and lines[i + 2].strip() == ""
            and lines[i + 3].strip() in valid_headers
            and lines[i + 4].strip() == ""
        ):
            blocks.append(i)
            i += 5
        else:
            i += 1
    return blocks


def classify(prev_line, next_line):
    prev_s = prev_line.rstrip("\n").rstrip()
    next_s = next_line.rstrip("\n").lstrip()

    if TERMINAL_RE.search(prev_s) or STRUCTURAL_NEXT_RE.match(next_s):
        return "paragraph-boundary", None

    if HYPHEN_BREAK_RE.search(prev_s) and re.match(r"^[a-z]", next_s):
        return "needs-review", "mid-word (hyphenated split)"

    if re.search(r"[a-zA-Z]$", prev_s) and re.match(r"^[a-z]", next_s):
        return "needs-review", "mid-sentence"

    return "needs-review", "uncertain boundary"


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--file", default="WHITEPAPER.md")
    ap.add_argument("--apply", action="store_true", help="Remove paragraph-boundary blocks in place")
    ap.add_argument("--report", default=None, help="Write the manual-review list to this path (Markdown)")
    args = ap.parse_args()

    with open(args.file, encoding="utf-8") as f:
        original_text = f.read()
    lines = original_text.split("\n")

    valid_headers = derive_valid_headers(lines)
    blocks = find_blocks(lines, valid_headers)

    results = []
    for start in blocks:
        page_num = lines[start + 1].strip()
        header = lines[start + 3].strip()
        prev_idx, next_idx = start - 1, start + 5
        prev_line = lines[prev_idx] if prev_idx >= 0 else ""
        next_line = lines[next_idx] if next_idx < len(lines) else ""
        kind, subtype = classify(prev_line, next_line)
        results.append(
            dict(
                start=start,
                page_num=page_num,
                header=header,
                prev_idx=prev_idx,
                next_idx=next_idx,
                prev_line=prev_line,
                next_line=next_line,
                kind=kind,
                subtype=subtype,
            )
        )

    safe = [r for r in results if r["kind"] == "paragraph-boundary"]
    review = [r for r in results if r["kind"] == "needs-review"]

    print(f"Scanned {args.file}: {len(lines)} lines")
    print(f"Found {len(results)} injected page-break blocks")
    print(f"  {len(safe)} paragraph-boundary (safe to delete)")
    print(f"  {len(review)} need manual review (mid-sentence/mid-word)")
    print()

    if args.apply:
        # Remove safe blocks from the end of the file backward so earlier
        # indices stay valid, collapsing each 5-line block to a single blank
        # line (the normal paragraph separator used throughout this file).
        new_lines = list(lines)
        for r in sorted(safe, key=lambda r: r["start"], reverse=True):
            s = r["start"]
            new_lines[s : s + 5] = [""]
        new_text = "\n".join(new_lines)
        with open(args.file, "w", encoding="utf-8") as f:
            f.write(new_text)
        print(f"Applied {len(safe)} paragraph-boundary removals to {args.file}")
        print(f"Line count: {len(lines)} -> {len(new_lines)}")
        print()

    if review:
        report_lines = [
            "# Whitepaper page-header removal: manual review needed",
            "",
            f"{len(review)} of {len(results)} injected page-break blocks land "
            "mid-sentence or mid-word and were NOT auto-resolved. For each, "
            "the text immediately before and after the injected "
            "`<blank>/<page number>/<blank>/<running header>/<blank>` block "
            "is shown so the original wording can be reconstructed by hand.",
            "",
        ]
        for r in review:
            report_lines.append(f"## Page {r['page_num']} / \"{r['header']}\" (line {r['start'] + 1})")
            report_lines.append(f"- Classification: {r['subtype']}")
            report_lines.append("- Before (last line before the block):")
            report_lines.append(f"  ```\n  {r['prev_line'].rstrip()}\n  ```")
            report_lines.append("- After (first line after the block):")
            report_lines.append(f"  ```\n  {r['next_line'].rstrip()}\n  ```")
            report_lines.append("")
        report_text = "\n".join(report_lines)
        if args.report:
            with open(args.report, "w", encoding="utf-8") as f:
                f.write(report_text)
            print(f"Manual-review list written to {args.report}")
        else:
            print(report_text)

    return 0


if __name__ == "__main__":
    sys.exit(main())
