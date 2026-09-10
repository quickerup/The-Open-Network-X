#!/usr/bin/env python3
"""Reject references to whitepaper sections that do not exist."""
from pathlib import Path
import re, sys
root = Path(__file__).resolve().parents[1]
whitepaper = (root / "WHITEPAPER.md").read_text(encoding="utf-8")
sections = set(re.findall(r"^#{1,6}\s+(\d+(?:\.\d+)+)\b", whitepaper, re.M))
pattern = re.compile(r"whitepaper\.md\s+§(\d+(?:\.\d+)+)(?:\s*[–-]\s*§?(\d+(?:\.\d+)+))?")
errors = []
for base in (root / "docs/specification", root / "crates"):
    for path in base.rglob("*"):
        if path.suffix not in {".md", ".rs"}: continue
        for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            for first, last in pattern.findall(line):
                for section in (first, last) if last else (first,):
                    if section not in sections:
                        errors.append(f"{path.relative_to(root)}:{n}: WHITEPAPER.md §{section} does not exist")
if errors:
    print("\n".join(errors), file=sys.stderr); sys.exit(1)
print("Spec citations reference existing whitepaper sections.")
