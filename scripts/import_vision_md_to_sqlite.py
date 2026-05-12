#!/usr/bin/env python3
"""DEPRECATED: Import standardized SC-100 vision Markdown batches into SQLite.

Use .github/skills/sc100-vision-md/scripts/import_question_md_to_sqlite.py instead.
This legacy importer writes an older schema and does not fully support the unified
interaction_options / interaction_targets model consumed by current TauriExam.
"""

from __future__ import annotations

import argparse
import re
import sqlite3
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

TITLE_RE = re.compile(r"^#\s+(.+?)\s*$", re.MULTILINE)
TITLE_QUESTIONS_RE = re.compile(r"Questions\s+(\d+)\s*[–-]\s*(\d+)", re.IGNORECASE)
BATCH_PAGES_RE = re.compile(r"处理页范围：PDF pages\s+(\d+)\s*[–-]\s*(\d+)")
PDF_RE = re.compile(r"来源 PDF：`([^`]+)`")
QUESTION_RE = re.compile(r"^##\s+Question\s+(\d+)\s*$", re.MULTILINE)
META_RE = re.compile(r"^-\s*([^:]+):\s*(.+?)\s*$", re.MULTILINE)
SECTION_RE = re.compile(r"^###\s+(.+?)\s*$", re.MULTILINE)
PAGE_RE = re.compile(r"(\d+)\s*[–-]\s*(\d+)|(\d+)")
OPTION_RE = re.compile(r"^([A-Z])\.\s+(.+?)(?:\s{2,})?$", re.MULTILINE)
TABLE_ROW_RE = re.compile(r"^\|(.+)\|\s*$", re.MULTILINE)
QUESTION_FILE_RE = re.compile(r"sc-100-questions-(\d+)-(\d+)\.md$", re.IGNORECASE)


@dataclass
class ParsedQuestion:
    id: str
    exam: str
    sequence_number: int
    source_question_number: int
    topic: str | None
    question_type: str
    status: str
    source_pages: str | None
    page_from: int | None
    page_to: int | None
    question_text: str
    options_md: str
    answer_area_md: str
    source_answer: str
    recommended_answer: str
    chinese_judgement: str
    reasoning: str
    notes: str
    question_md: str
    md_file: str
    pdf_file: str | None
    batch_id: str
    batch_order: int


@dataclass
class ParsedBatch:
    id: str
    md_file: str
    title: str
    pdf_file: str | None
    question_from: int | None
    question_to: int | None
    page_from: int | None
    page_to: int | None
    carryover: str
    questions: list[ParsedQuestion]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Import SC-100 vision Markdown files into SQLite.")
    parser.add_argument("--input", default="output/vision-md", help="Root folder containing sc-100-questions-*.md files.")
    parser.add_argument("--db", default="output/vision-db/sc-100.sqlite", help="SQLite database path.")
    parser.add_argument("--exam", default="SC-100", help="Exam code.")
    parser.add_argument("--reset", action="store_true", help="Drop and recreate tables before importing.")
    return parser.parse_args()


def normalize_heading(value: str) -> str:
    return value.strip().lower().replace("（", "(").replace("）", ")")


def clean_text(value: str) -> str:
    return value.strip().replace("\r\n", "\n").replace("\r", "\n")


def section_map(block: str) -> dict[str, str]:
    matches = list(SECTION_RE.finditer(block))
    sections: dict[str, str] = {}
    for index, match in enumerate(matches):
        start = match.end()
        end = matches[index + 1].start() if index + 1 < len(matches) else len(block)
        sections[normalize_heading(match.group(1))] = clean_text(block[start:end])
    return sections


def meta_map(block: str) -> dict[str, str]:
    body_before_sections = block.split("\n### ", 1)[0]
    return {key.strip().lower(): value.strip() for key, value in META_RE.findall(body_before_sections)}


def parse_pages(value: str | None) -> tuple[int | None, int | None]:
    if not value:
        return None, None
    match = PAGE_RE.search(value)
    if not match:
        return None, None
    if match.group(3):
        page = int(match.group(3))
        return page, page
    return int(match.group(1)), int(match.group(2))


def parse_options(options_md: str) -> list[tuple[str, str]]:
    options: list[tuple[str, str]] = []
    lines = options_md.splitlines()
    current_key: str | None = None
    current_lines: list[str] = []
    for line in lines:
        match = re.match(r"^([A-Z])\.\s+(.+)$", line.strip())
        if match:
            if current_key:
                options.append((current_key, clean_text("\n".join(current_lines))))
            current_key = match.group(1)
            current_lines = [match.group(2)]
        elif current_key and line.strip():
            current_lines.append(line.strip())
    if current_key:
        options.append((current_key, clean_text("\n".join(current_lines))))
    return options


def parse_answer_area(answer_area_md: str) -> list[tuple[str, str, str | None]]:
    rows: list[tuple[str, str, str | None]] = []
    for match in TABLE_ROW_RE.finditer(answer_area_md):
        cells = [cell.strip() for cell in match.group(1).split("|")]
        if len(cells) < 2 or set("".join(cells)) <= {"-", ":"}:
            continue
        if cells[0].lower() in {"prompt", "requirement"}:
            continue
        source = cells[1] if len(cells) > 2 else None
        recommended = cells[2] if len(cells) > 2 else cells[1]
        rows.append((cells[0], recommended, source))
    return rows


def parse_batch(path: Path, root: Path, exam: str) -> ParsedBatch:
    text = path.read_text(encoding="utf-8")
    title = TITLE_RE.search(text).group(1) if TITLE_RE.search(text) else path.stem
    pdf_match = PDF_RE.search(text)
    range_match = TITLE_QUESTIONS_RE.search(title) or TITLE_QUESTIONS_RE.search(path.name)
    batch_pages = BATCH_PAGES_RE.search(text)
    carryover = "\n".join(line.strip() for line in text.splitlines() if "carryover" in line.lower())
    batch_id = path.stem

    question_matches = list(QUESTION_RE.finditer(text))
    questions: list[ParsedQuestion] = []
    for index, match in enumerate(question_matches):
        q_number = int(match.group(1))
        start = match.start()
        end = question_matches[index + 1].start() if index + 1 < len(question_matches) else len(text)
        block = clean_text(text[start:end])
        sections = section_map(block)
        metas = meta_map(block)
        source_pages = metas.get("source pages")
        page_from, page_to = parse_pages(source_pages)
        q_type = metas.get("type", "unknown")
        status = metas.get("status", "unknown")
        sequence_number = q_number
        qid = f"{exam.lower()}-q{sequence_number:04}"

        questions.append(
            ParsedQuestion(
                id=qid,
                exam=exam,
                sequence_number=sequence_number,
                source_question_number=q_number,
                topic=metas.get("topic"),
                question_type=q_type,
                status=status,
                source_pages=source_pages,
                page_from=page_from,
                page_to=page_to,
                question_text=sections.get("question", ""),
                options_md=sections.get("options", ""),
                answer_area_md=sections.get("answer area", ""),
                source_answer=sections.get("source answer", sections.get("correct answer", "")),
                recommended_answer=sections.get("my recommended answer", ""),
                chinese_judgement=sections.get("我的判断(中文)", ""),
                reasoning=sections.get("reasoning", ""),
                notes=sections.get("notes", ""),
                question_md=block,
                md_file=str(path.relative_to(Path.cwd()) if path.is_absolute() else path),
                pdf_file=pdf_match.group(1) if pdf_match else None,
                batch_id=batch_id,
                batch_order=index + 1,
            )
        )

    return ParsedBatch(
        id=batch_id,
        md_file=str(path.relative_to(Path.cwd()) if path.is_absolute() else path),
        title=title,
        pdf_file=pdf_match.group(1) if pdf_match else None,
        question_from=int(range_match.group(1)) if range_match else None,
        question_to=int(range_match.group(2)) if range_match else None,
        page_from=int(batch_pages.group(1)) if batch_pages else None,
        page_to=int(batch_pages.group(2)) if batch_pages else None,
        carryover=carryover,
        questions=questions,
    )


def create_schema(conn: sqlite3.Connection, reset: bool) -> None:
    if reset:
        conn.executescript(
            """
            DROP TABLE IF EXISTS answer_areas;
            DROP TABLE IF EXISTS options;
            DROP TABLE IF EXISTS questions;
            DROP TABLE IF EXISTS markdown_batches;
            """
        )
    conn.executescript(
        """
        CREATE TABLE IF NOT EXISTS markdown_batches (
          id TEXT PRIMARY KEY,
          md_file TEXT NOT NULL UNIQUE,
          title TEXT NOT NULL,
          pdf_file TEXT,
          question_from INTEGER,
          question_to INTEGER,
          page_from INTEGER,
          page_to INTEGER,
          carryover TEXT,
          question_count INTEGER NOT NULL,
          imported_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS questions (
          id TEXT PRIMARY KEY,
          exam TEXT NOT NULL,
          sequence_number INTEGER NOT NULL UNIQUE,
          source_question_number INTEGER,
          topic TEXT,
          question_type TEXT NOT NULL,
          status TEXT NOT NULL,
          source_pages TEXT,
          page_from INTEGER,
          page_to INTEGER,
          question_text TEXT NOT NULL,
          options_md TEXT,
          answer_area_md TEXT,
          source_answer TEXT,
          recommended_answer TEXT,
          chinese_judgement TEXT,
          reasoning TEXT,
          notes TEXT,
          question_md TEXT NOT NULL,
          md_file TEXT NOT NULL,
          pdf_file TEXT,
          batch_id TEXT NOT NULL,
          batch_order INTEGER NOT NULL,
          imported_at TEXT NOT NULL,
          FOREIGN KEY(batch_id) REFERENCES markdown_batches(id)
        );

        CREATE TABLE IF NOT EXISTS options (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          question_id TEXT NOT NULL,
          option_key TEXT NOT NULL,
          option_text TEXT NOT NULL,
          sort_order INTEGER NOT NULL,
          FOREIGN KEY(question_id) REFERENCES questions(id)
        );

        CREATE TABLE IF NOT EXISTS answer_areas (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          question_id TEXT NOT NULL,
          prompt TEXT NOT NULL,
          source_selection TEXT,
          recommended_selection TEXT NOT NULL,
          sort_order INTEGER NOT NULL,
          FOREIGN KEY(question_id) REFERENCES questions(id)
        );

        CREATE INDEX IF NOT EXISTS idx_questions_pages ON questions(page_from, page_to);
        CREATE INDEX IF NOT EXISTS idx_questions_type_status ON questions(question_type, status);
        CREATE INDEX IF NOT EXISTS idx_options_question ON options(question_id);
        CREATE INDEX IF NOT EXISTS idx_answer_areas_question ON answer_areas(question_id);
        """
    )


def import_batches(conn: sqlite3.Connection, batches: list[ParsedBatch]) -> None:
    now = datetime.now(timezone.utc).isoformat()
    for batch in batches:
        conn.execute(
            """
            INSERT OR REPLACE INTO markdown_batches
            (id, md_file, title, pdf_file, question_from, question_to, page_from, page_to, carryover, question_count, imported_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                batch.id,
                batch.md_file,
                batch.title,
                batch.pdf_file,
                batch.question_from,
                batch.question_to,
                batch.page_from,
                batch.page_to,
                batch.carryover,
                len(batch.questions),
                now,
            ),
        )
        for question in batch.questions:
            conn.execute("DELETE FROM options WHERE question_id = ?", (question.id,))
            conn.execute("DELETE FROM answer_areas WHERE question_id = ?", (question.id,))
            conn.execute(
                """
                INSERT OR REPLACE INTO questions
                (id, exam, sequence_number, source_question_number, topic, question_type, status, source_pages, page_from, page_to,
                 question_text, options_md, answer_area_md, source_answer, recommended_answer, chinese_judgement, reasoning, notes,
                 question_md, md_file, pdf_file, batch_id, batch_order, imported_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    question.id,
                    question.exam,
                    question.sequence_number,
                    question.source_question_number,
                    question.topic,
                    question.question_type,
                    question.status,
                    question.source_pages,
                    question.page_from,
                    question.page_to,
                    question.question_text,
                    question.options_md,
                    question.answer_area_md,
                    question.source_answer,
                    question.recommended_answer,
                    question.chinese_judgement,
                    question.reasoning,
                    question.notes,
                    question.question_md,
                    question.md_file,
                    question.pdf_file,
                    question.batch_id,
                    question.batch_order,
                    now,
                ),
            )
            for order, (key, text) in enumerate(parse_options(question.options_md), start=1):
                conn.execute(
                    "INSERT INTO options (question_id, option_key, option_text, sort_order) VALUES (?, ?, ?, ?)",
                    (question.id, key, text, order),
                )
            for order, (prompt, recommended, source) in enumerate(parse_answer_area(question.answer_area_md), start=1):
                conn.execute(
                    "INSERT INTO answer_areas (question_id, prompt, source_selection, recommended_selection, sort_order) VALUES (?, ?, ?, ?, ?)",
                    (question.id, prompt, source, recommended, order),
                )


def main() -> None:
    args = parse_args()
    root = Path(args.input)
    db_path = Path(args.db)
    files = sorted(root.rglob("sc-100-questions-*.md"))
    batches = [parse_batch(path, root, args.exam) for path in files]

    db_path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(db_path)
    try:
        create_schema(conn, args.reset)
        import_batches(conn, batches)
        conn.commit()
    finally:
        conn.close()

    question_count = sum(len(batch.questions) for batch in batches)
    print(f"Imported {question_count} question(s) from {len(batches)} markdown batch file(s) into {db_path}")


if __name__ == "__main__":
    main()
