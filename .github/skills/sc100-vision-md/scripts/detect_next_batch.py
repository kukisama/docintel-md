#!/usr/bin/env python3
"""Detect the next SC-100 vision-reading batch from existing Markdown outputs."""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path

import fitz  # PyMuPDF

PAGE_RANGE_RE = re.compile(r"PDF pages\s+(\d+)\s*[–-]\s*(\d+)")
TITLE_QUESTION_RANGE_RE = re.compile(r"Questions\s+(\d+)\s*[–-]\s*(\d+)", re.IGNORECASE)
QUESTION_HEADING_RE = re.compile(r"^##\s+Question\s+(\d+)\s*$", re.MULTILINE)


@dataclass
class Batch:
    path: Path
    page_from: int
    page_to: int
    question_from: int | None
    question_to: int | None
    question_headings: list[int]
    carryover_lines: list[str]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Detect next PDF page/question batch for sc100-vision-md.")
    parser.add_argument("--vision-md-root", default="output/vision-md", help="Root containing existing vision Markdown batches.")
    parser.add_argument("--vision-pages-root", default="output/vision-pages", help="Root for rendered page images.")
    parser.add_argument("--pdf", default=None, help="Optional PDF path. If omitted, uses the first PDF in the workspace root.")
    parser.add_argument("--pages-per-batch", type=int, default=30, help="Suggested page window for the next pass. Default: 30")
    parser.add_argument("--exam-slug", default="sc-100", help="Slug used in output path names. Default: sc-100")
    return parser.parse_args()


def parse_batch(path: Path) -> Batch | None:
    text = path.read_text(encoding="utf-8")
    page_match = PAGE_RANGE_RE.search(text)
    if not page_match:
        return None

    title_match = TITLE_QUESTION_RANGE_RE.search(text)
    headings = [int(value) for value in QUESTION_HEADING_RE.findall(text)]
    carryover = [line.strip() for line in text.splitlines() if "carryover" in line.lower() or "Carryover" in line]

    return Batch(
        path=path,
        page_from=int(page_match.group(1)),
        page_to=int(page_match.group(2)),
        question_from=int(title_match.group(1)) if title_match else (min(headings) if headings else None),
        question_to=int(title_match.group(2)) if title_match else (max(headings) if headings else None),
        question_headings=headings,
        carryover_lines=carryover,
    )


def find_pdf(explicit: str | None) -> str | None:
    if explicit:
        return explicit
    pdfs = sorted(path for path in Path(".").glob("*.pdf") if path.is_file())
    return str(pdfs[0]) if pdfs else None


def pdf_page_count(pdf: str | None) -> int | None:
    if not pdf:
        return None
    path = Path(pdf)
    if not path.exists():
        return None
    with fitz.open(str(path)) as doc:
        return doc.page_count


def main() -> None:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")

    args = parse_args()
    root = Path(args.vision_md_root)
    batches = []
    if root.exists():
        for path in sorted(root.rglob("*.md")):
            batch = parse_batch(path)
            if batch:
                batches.append(batch)

    if batches:
        latest = max(batches, key=lambda batch: (batch.page_to, batch.question_to or 0, str(batch.path)))
        next_page_from = latest.page_to + 1
        previous_question_to = latest.question_to
        previous_carryover = latest.carryover_lines
    else:
        latest = None
        next_page_from = 1
        previous_question_to = None
        previous_carryover = []

    pdf = find_pdf(args.pdf)
    total_pages = pdf_page_count(pdf)
    done = total_pages is not None and next_page_from > total_pages
    next_page_to = next_page_from + args.pages_per_batch - 1
    if total_pages is not None:
        next_page_to = min(next_page_to, total_pages)
    page_dir = f"{args.exam_slug}-pages-{next_page_from:03}-{next_page_to:03}"
    output = {
        "done": done,
        "done_reason": "all PDF pages are already covered by existing Markdown batches" if done else None,
        "pdf": pdf,
        "pdf_page_count": total_pages,
        "existing_batches": len(batches),
        "latest_markdown": str(latest.path) if latest else None,
        "latest_page_range": [latest.page_from, latest.page_to] if latest else None,
        "latest_question_range": [latest.question_from, latest.question_to] if latest else None,
        "previous_carryover": previous_carryover,
        "next_page_range": None if done else [next_page_from, next_page_to],
        "expected_next_question_start": None if done else ((previous_question_to + 1) if previous_question_to else None),
        "suggested_pages_output": None if done else str(Path(args.vision_pages_root) / page_dir),
        "suggested_text_output": None if done else str(Path(args.vision_pages_root) / page_dir / f"page-text-{next_page_from:03}-{next_page_to:03}.txt"),
        "suggested_md_output_dir": None if done else str(Path(args.vision_md_root) / page_dir),
        "notes": [
            "If done is true, do not render, extract, or write anything. Stop the current turn as complete.",
            "Render/extract this suggested page range, then inspect actual Question boundaries visually.",
            "If previous_carryover is non-empty, attach pre-next-question continuation to the previous question before starting the new batch.",
            "Do not trust the expected_next_question_start blindly; verify visible Question headings on rendered pages.",
        ],
    }
    print(json.dumps(output, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
