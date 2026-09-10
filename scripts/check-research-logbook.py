#!/usr/bin/env python3
import sys
import os
import re
import subprocess

LOGBOOK_PATH = "docs/planning/research-logbook.md"

def get_entry_count(file_content):
    entry_pattern = re.compile(r"^###\s+Entry\s+#(\d+)", re.MULTILINE)
    matches = list(entry_pattern.finditer(file_content))
    return len(matches), matches, entry_pattern

def main():
    if not os.path.exists(LOGBOOK_PATH):
        print(f"ERROR: Logbook file '{LOGBOOK_PATH}' does not exist.")
        sys.exit(1)

    with open(LOGBOOK_PATH, "r", encoding="utf-8") as f:
        content = f.read()

    # Split by entry headings: ### Entry #N
    entry_count, matches, _ = get_entry_count(content)

    if not matches:
        print("ERROR: No entries found in research logbook. Expected headings like '### Entry #N'.")
        sys.exit(1)

    entries = []
    for i in range(len(matches)):
        start = matches[i].start()
        end = matches[i+1].start() if i + 1 < len(matches) else len(content)
        entry_num = int(matches[i].group(1))
        entry_text = content[start:end]
        entries.append((entry_num, entry_text))

    expected_num = 1
    for entry_num, entry_text in entries:
        if entry_num != expected_num:
            print(f"ERROR: Non-sequential entry number. Expected Entry #{expected_num}, but found Entry #{entry_num}.")
            sys.exit(1)
        expected_num += 1

        # Check for [ANSWER] and [QUESTION] keywords / headings
        has_answer = bool(re.search(r"\[ANSWER\]", entry_text))
        has_question = bool(re.search(r"\[QUESTION\]", entry_text))

        if not has_answer:
            print(f"ERROR: Entry #{entry_num} is missing required '[ANSWER]' label.")
            sys.exit(1)

        if not has_question:
            print(f"ERROR: Entry #{entry_num} is missing required '[QUESTION]' label.")
            sys.exit(1)

        # Check that answer and question sections have content
        answer_match = re.search(r"\[ANSWER\]\s*\n+(.*?)(?=\n+#+|\Z)", entry_text, re.DOTALL)
        question_match = re.search(r"\[QUESTION\]\s*\n+(.*?)(?=\n+#+|\Z)", entry_text, re.DOTALL)

        if not answer_match or not answer_match.group(1).strip():
            print(f"ERROR: Entry #{entry_num} has an empty '[ANSWER]' section.")
            sys.exit(1)

        if not question_match or not question_match.group(1).strip():
            print(f"ERROR: Entry #{entry_num} has an empty '[QUESTION]' section.")
            sys.exit(1)

    # In CI / PR verification, if comparing against main branch, ensure new entry is present if logbook was established
    check_pr_diff = os.getenv("CHECK_PR_DIFF", "false").lower() in ("true", "1", "yes")
    if check_pr_diff:
        try:
            main_content = subprocess.check_output(["git", "show", "origin/main:" + LOGBOOK_PATH], text=True, stderr=subprocess.DEVNULL)
            main_count, _, _ = get_entry_count(main_content)
            if entry_count <= main_count:
                print(f"ERROR: PR does not add a new entry to '{LOGBOOK_PATH}'. Main has {main_count} entries, PR has {entry_count} entries.")
                sys.exit(1)
        except Exception:
            # If logbook doesn't exist on main yet (e.g. this PR creates it), skip diff check
            pass

    print(f"SUCCESS: Research logbook '{LOGBOOK_PATH}' passed all checks ({len(entries)} entries verified).")
    sys.exit(0)

if __name__ == "__main__":
    main()
