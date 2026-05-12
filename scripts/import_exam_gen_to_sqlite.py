#!/usr/bin/env python3
"""DEPRECATED: Import AI-generated exam Markdown batches into SQLite for TauriExam.

Use .github/skills/exam-question-gen/scripts/import_question_md_to_sqlite.py instead.
This legacy importer is kept only for historical compatibility.

Usage:
    python scripts/import_exam_gen_to_sqlite.py --input output/exam-gen/AI-900
    python scripts/import_exam_gen_to_sqlite.py --input output/exam-gen/AZ-104 --db question-banks/AZ-104.sqlite
    python scripts/import_exam_gen_to_sqlite.py --input output/exam-gen/AI-900 --reset
"""

from __future__ import annotations

import argparse
import re
import sqlite3
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path


# --- Regex patterns for exam-gen Markdown format ---

QUESTION_HEADING_RE = re.compile(r"^##\s+Question\s+(\d+)\s*$", re.MULTILINE)
SECTION_RE = re.compile(r"^###\s+(.+?)\s*$", re.MULTILINE)
META_RE = re.compile(r"^-\s*([^:]+):\s*(.+?)\s*$", re.MULTILINE)
OPTION_RE = re.compile(r"^([A-Z])\.\s+(.+)$", re.MULTILINE)
TABLE_ROW_RE = re.compile(r"^\|(.+)\|\s*$", re.MULTILINE)


@dataclass
class ParsedQuestion:
    id: str
    exam: str
    sequence_number: int
    topic: str
    question_type: str
    difficulty: str
    question_text: str
    options_md: str
    answer_area_md: str
    correct_answer: str
    chinese_explanation: str
    key_concept: str
    question_md: str
    md_file: str
    batch_order: int


@dataclass
class ParsedBatch:
    md_file: str
    questions: list[ParsedQuestion] = field(default_factory=list)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Import AI-generated exam Markdown files into SQLite for TauriExam."
    )
    parser.add_argument(
        "--input",
        required=True,
        help="Folder containing questions-*.md files (e.g. output/exam-gen/AI-900).",
    )
    parser.add_argument(
        "--db",
        default=None,
        help="SQLite output path. Defaults to question-banks/<EXAM>.sqlite based on folder name.",
    )
    parser.add_argument(
        "--reset",
        action="store_true",
        help="Drop and recreate tables before importing.",
    )
    return parser.parse_args()


def normalize_heading(value: str) -> str:
    return (
        value.strip()
        .lower()
        .replace("（", "(")
        .replace("）", ")")
    )


def clean_text(value: str) -> str:
    return value.strip().replace("\r\n", "\n").replace("\r", "\n")


def section_map(block: str) -> dict[str, str]:
    """Extract ### sections from a question block."""
    matches = list(SECTION_RE.finditer(block))
    sections: dict[str, str] = {}
    for index, match in enumerate(matches):
        start = match.end()
        end = matches[index + 1].start() if index + 1 < len(matches) else len(block)
        sections[normalize_heading(match.group(1))] = clean_text(block[start:end])
    return sections


def meta_map(block: str) -> dict[str, str]:
    """Extract - Key: Value metadata lines before the first ### section."""
    body_before_sections = block.split("\n### ", 1)[0]
    return {key.strip().lower(): value.strip() for key, value in META_RE.findall(body_before_sections)}


def parse_options(options_md: str) -> list[tuple[str, str]]:
    """Parse A. xxx / B. xxx style options."""
    options: list[tuple[str, str]] = []
    current_key: str | None = None
    current_lines: list[str] = []
    for line in options_md.splitlines():
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


def parse_answer_area(answer_area_md: str) -> list[tuple[str, str]]:
    """Parse table rows from Answer Area / Statements sections."""
    rows: list[tuple[str, str]] = []
    for match in TABLE_ROW_RE.finditer(answer_area_md):
        cells = [cell.strip() for cell in match.group(1).split("|")]
        if len(cells) < 2:
            continue
        # Skip header separators (---) and header rows
        if set("".join(cells)) <= {"-", ":", " "}:
            continue
        if cells[0].lower() in {"提示/位置", "prompt", "陈述", "序号"}:
            continue
        rows.append((cells[0], cells[1]))
    return rows


def parse_batch(path: Path, exam: str, global_offset: int) -> ParsedBatch:
    """Parse a single Markdown file into a batch of questions."""
    text = path.read_text(encoding="utf-8")
    batch = ParsedBatch(md_file=str(path))

    question_matches = list(QUESTION_HEADING_RE.finditer(text))
    for index, match in enumerate(question_matches):
        q_number = int(match.group(1))
        start = match.start()
        end = question_matches[index + 1].start() if index + 1 < len(question_matches) else len(text)
        block = clean_text(text[start:end])

        sections = section_map(block)
        metas = meta_map(block)

        topic = metas.get("topic", "")
        q_type = metas.get("type", "single_choice")
        difficulty = metas.get("difficulty", "medium")

        # Determine options vs answer area
        options_md = sections.get("options", "")
        answer_area_md = sections.get("answer area", sections.get("statements", ""))

        # Correct answer: try multiple heading variants
        correct_answer = (
            sections.get("correct answer", "")
            or sections.get("正确答案", "")
        )

        # Chinese explanation
        chinese_explanation = (
            sections.get("解析(中文)", "")
            or sections.get("解析（中文）", "")
            or sections.get("解析", "")
        )

        key_concept = sections.get("key concept", "")

        sequence_number = global_offset + index + 1
        qid = f"{exam.lower()}-q{sequence_number:04}"

        batch.questions.append(
            ParsedQuestion(
                id=qid,
                exam=exam,
                sequence_number=sequence_number,
                topic=topic,
                question_type=q_type,
                difficulty=difficulty,
                question_text=sections.get("question", ""),
                options_md=options_md,
                answer_area_md=answer_area_md,
                correct_answer=correct_answer,
                chinese_explanation=chinese_explanation,
                key_concept=key_concept,
                question_md=block,
                md_file=str(path.name),
                batch_order=index + 1,
            )
        )

    return batch


def create_schema(conn: sqlite3.Connection, reset: bool) -> None:
    """Create TauriExam-compatible schema."""
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
          batch_id TEXT,
          batch_order INTEGER NOT NULL,
          imported_at TEXT NOT NULL
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

        CREATE INDEX IF NOT EXISTS idx_questions_type_status ON questions(question_type, status);
        CREATE INDEX IF NOT EXISTS idx_options_question ON options(question_id);
        CREATE INDEX IF NOT EXISTS idx_answer_areas_question ON answer_areas(question_id);
        """
    )


def import_questions(conn: sqlite3.Connection, batches: list[ParsedBatch]) -> int:
    """Insert parsed questions into SQLite. Returns total count."""
    now = datetime.now(timezone.utc).isoformat()
    total = 0

    for batch in batches:
        for question in batch.questions:
            conn.execute("DELETE FROM options WHERE question_id = ?", (question.id,))
            conn.execute("DELETE FROM answer_areas WHERE question_id = ?", (question.id,))
            conn.execute(
                """
                INSERT OR REPLACE INTO questions
                (id, exam, sequence_number, source_question_number, topic, question_type, status,
                 source_pages, page_from, page_to,
                 question_text, options_md, answer_area_md,
                 source_answer, recommended_answer, chinese_judgement, reasoning, notes,
                 question_md, md_file, pdf_file, batch_id, batch_order, imported_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    question.id,
                    question.exam,
                    question.sequence_number,
                    question.sequence_number,  # source_question_number = sequence
                    question.topic,
                    question.question_type,
                    "parsed",  # status: AI-generated are always clean
                    None,  # source_pages: N/A
                    None,  # page_from: N/A
                    None,  # page_to: N/A
                    question.question_text,
                    question.options_md if question.options_md else None,
                    question.answer_area_md if question.answer_area_md else None,
                    question.correct_answer,  # source_answer = correct answer
                    question.correct_answer,  # recommended_answer = same
                    question.chinese_explanation,  # chinese_judgement
                    question.key_concept,  # reasoning = key concept
                    f"difficulty: {question.difficulty}",  # notes
                    question.question_md,
                    question.md_file,
                    None,  # pdf_file: N/A
                    question.md_file,  # batch_id = filename
                    question.batch_order,
                    now,
                ),
            )

            # Insert options
            for order, (key, text) in enumerate(parse_options(question.options_md), start=1):
                conn.execute(
                    "INSERT INTO options (question_id, option_key, option_text, sort_order) VALUES (?, ?, ?, ?)",
                    (question.id, key, text, order),
                )

            # Insert answer area rows (for hotspot/drag_drop/yes_no_series)
            for order, (prompt, selection) in enumerate(parse_answer_area(question.answer_area_md), start=1):
                conn.execute(
                    "INSERT INTO answer_areas (question_id, prompt, source_selection, recommended_selection, sort_order) VALUES (?, ?, ?, ?, ?)",
                    (question.id, prompt, selection, selection, order),
                )

            total += 1

    return total


def main() -> None:
    args = parse_args()
    root = Path(args.input)

    if not root.exists():
        print(f"Error: Input folder does not exist: {root}")
        return

    # Derive exam code from folder name (e.g. output/exam-gen/AI-900 -> AI-900)
    exam = root.name.upper()

    # Default db path
    db_path = Path(args.db) if args.db else Path(f"question-banks/{exam}.sqlite")

    # Find all question markdown files
    files = sorted(root.glob("questions-*.md"))
    if not files:
        # Also try nested patterns
        files = sorted(root.rglob("questions-*.md"))
    if not files:
        print(f"Error: No questions-*.md files found in {root}")
        return

    print(f"Exam: {exam}")
    print(f"Found {len(files)} markdown file(s) in {root}")
    print(f"Output: {db_path}")

    # Parse all batches with sequential numbering
    batches: list[ParsedBatch] = []
    global_offset = 0
    for path in files:
        batch = parse_batch(path, exam, global_offset)
        batches.append(batch)
        global_offset += len(batch.questions)

    # Write to SQLite
    db_path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(str(db_path))
    try:
        create_schema(conn, args.reset)
        total = import_questions(conn, batches)
        conn.commit()
    finally:
        conn.close()

    print(f"Done: imported {total} question(s) from {len(batches)} file(s) into {db_path}")


if __name__ == "__main__":
    main()
