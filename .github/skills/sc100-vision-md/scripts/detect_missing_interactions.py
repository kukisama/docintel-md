"""Detect hotspot/drag_drop questions missing Interaction Options/Targets.

Outputs JSON array of batch tasks, each = one MD file + its missing questions.
Usage:
  python .github/skills/sc100-vision-md/scripts/detect_missing_interactions.py
  python ... --db question-banks/SC-100.sqlite
"""

import argparse
import json
import sqlite3
import os
import sys

def main():
    # Ensure UTF-8 output
    sys.stdout.reconfigure(encoding='utf-8')
    parser = argparse.ArgumentParser()
    parser.add_argument("--db", default="question-banks/SC-100.sqlite")
    args = parser.parse_args()

    if not os.path.exists(args.db):
        print(json.dumps({"error": f"DB not found: {args.db}"}))
        sys.exit(1)

    conn = sqlite3.connect(args.db)
    conn.row_factory = sqlite3.Row

    # Get all hotspot/drag_drop questions
    questions = conn.execute("""
        SELECT id, source_question_number, question_type, source_pages,
               md_file, pdf_file
        FROM questions
        WHERE question_type IN ('hotspot', 'drag_drop')
        ORDER BY source_question_number
    """).fetchall()

    # Get questions that already have interaction data
    has_interaction = set()
    for row in conn.execute("SELECT DISTINCT question_id FROM interaction_options"):
        has_interaction.add(row[0])

    # Build batch tasks grouped by md_file
    batches = {}
    for q in questions:
        qid = q["id"]
        if qid in has_interaction:
            continue
        md_file = q["md_file"]
        if md_file not in batches:
            batches[md_file] = {
                "md_file": md_file,
                "pdf_file": q["pdf_file"],
                "questions": [],
            }
        batches[md_file]["questions"].append({
            "id": qid,
            "number": q["source_question_number"],
            "type": q["question_type"],
            "source_pages": q["source_pages"],
        })

    conn.close()

    task_list = sorted(batches.values(), key=lambda b: b["questions"][0]["number"])

    # Summary
    total_missing = sum(len(b["questions"]) for b in task_list)
    result = {
        "done": total_missing == 0,
        "total_missing": total_missing,
        "total_batches": len(task_list),
        "batches": task_list,
    }

    if total_missing == 0:
        result["done_reason"] = "All hotspot/drag_drop questions have Interaction Options/Targets."

    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
