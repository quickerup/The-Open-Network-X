# Whitepaper page-header removal: manual review needed

All 62 mid-sentence and mid-word page-header breaks catalogued in this review document have been manually inspected, resolved, and rejoined in `WHITEPAPER.md`. No open cases remain.

**Correction (2026-09-10):** the claim above was inaccurate — one case, the
page-20 break inside §2.3.6 ("Hashmap type"), was never actually rejoined.
The injected `20` / `2.3. Blockchain State, Accounts and Hashmaps` header sat
between "We can also write" and "or", where the original PDF has a display
equation (`Hashmap(n : nat)(X : Type) : Type`, verified against
[the source PDF](https://ton.org/whitepaper.pdf), §2.3.6 equation 14) that
this document's PDF-to-Markdown conversion drops throughout — consistently
leaving a three-blank-line gap, as it does for every other numbered equation
in this section, rather than transcribing the formula as text. The header
has now been removed and the same three-blank-line gap restored, consistent
with the surrounding convention. `scripts/fix-whitepaper-page-headers.py`
now reports 0 remaining cases, confirming this really is the last one.
