#!/usr/bin/env python3
"""Import vision-read exam Markdown into a TauriExam SQLite question bank.

This script lives inside the skill folder intentionally. It keeps the PDF-reading
skill self-contained while producing the same SQLite schema as the AI-generated
question workflow, including drag_drop structures.
"""

from __future__ import annotations

import argparse
import re
import sqlite3
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path

TITLE_RE = re.compile(r"^#\s+(.+?)\s*$", re.MULTILINE)
TITLE_QUESTIONS_RE = re.compile(r"Questions\s+(\d+)\s*[–-]\s*(\d+)", re.IGNORECASE)
BATCH_PAGES_RE = re.compile(r"(?:处理页范围：)?PDF pages\s+(\d+)\s*[–-]\s*(\d+)", re.IGNORECASE)
PDF_RE = re.compile(r"来源 PDF：`([^`]+)`|Source PDF:\s*`([^`]+)`", re.IGNORECASE)
QUESTION_RE = re.compile(r"^##\s+Question\s+(\d+)\s*$", re.MULTILINE)
SECTION_RE = re.compile(r"^###\s+(.+?)\s*$", re.MULTILINE)
META_RE = re.compile(r"^-\s*([^:]+):\s*(.+?)\s*$", re.MULTILINE)
PAGE_RE = re.compile(r"(\d+)\s*[–-]\s*(\d+)|(\d+)")


@dataclass
class DragOption:
    id: str
    text: str
    group: str | None
    is_distractor: bool
    sort_order: int


@dataclass
class DragSlot:
    id: str
    label: str
    correct_option: str
    sort_order: int


@dataclass
class HotspotOption:
    id: str
    text: str
    group: str | None
    sort_order: int


@dataclass
class HotspotRow:
    id: str
    prompt: str
    option_group: str | None
    correct_option: str
    sort_order: int


@dataclass
class InteractionOption:
    id: str
    text: str
    group: str | None
    is_distractor: bool
    sort_order: int


@dataclass
class InteractionTarget:
    id: str
    position: int
    label: str
    option_group: str | None
    correct_option: str


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
    drag_options: list[DragOption] = field(default_factory=list)
    drag_slots: list[DragSlot] = field(default_factory=list)
    hotspot_options: list[HotspotOption] = field(default_factory=list)
    hotspot_rows: list[HotspotRow] = field(default_factory=list)
    interaction_options: list[InteractionOption] = field(default_factory=list)
    interaction_targets: list[InteractionTarget] = field(default_factory=list)


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
    parser = argparse.ArgumentParser(description="Import vision-read Markdown questions into SQLite.")
    parser.add_argument("--input", default="output/vision-md", help="Root folder containing vision Markdown files.")
    parser.add_argument("--db", default=None, help="SQLite output path. Defaults to question-banks/<EXAM>.sqlite.")
    parser.add_argument("--exam", default="SC-100", help="Exam code used for generated question ids.")
    parser.add_argument("--pattern", default="*questions-*.md", help="Markdown glob pattern under --input.")
    parser.add_argument("--reset", action="store_true", help="Drop and recreate tables before importing.")
    return parser.parse_args()


def normalize_heading(value: str) -> str:
    return value.strip().lower().replace("（", "(").replace("）", ")")


def clean_text(value: str) -> str:
    return value.strip().replace("\r\n", "\n").replace("\r", "\n")


def exam_id_prefix(exam: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", exam.lower()).strip("-")


def normalize_question_type(value: str | None) -> str:
    normalized = (value or "unknown").strip().lower().replace(" ", "_").replace("-", "_")
    if "drag" in normalized or "drop" in normalized:
        return "drag_drop"
    if "hotspot" in normalized or "hot_spot" in normalized:
        return "hotspot"
    if "multiple" in normalized or normalized in {"multi", "multi_choice"}:
        return "multiple_choice"
    if "single" in normalized:
        return "single_choice"
    return normalized or "unknown"


def display_path(path: Path) -> str:
    if not path.is_absolute():
        return str(path)
    try:
        return str(path.relative_to(Path.cwd()))
    except ValueError:
        return str(path)


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


def parse_markdown_table(section_md: str) -> list[dict[str, str]]:
    rows: list[list[str]] = []
    for line in section_md.splitlines():
        line = line.strip()
        if not line.startswith("|") or not line.endswith("|"):
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if not cells or set("".join(cells)) <= {"-", ":", " "}:
            continue
        rows.append(cells)
    if len(rows) < 2:
        return []
    headers = [normalize_heading(cell) for cell in rows[0]]
    output: list[dict[str, str]] = []
    for cells in rows[1:]:
        if len(cells) < len(headers):
            cells += [""] * (len(headers) - len(cells))
        output.append({headers[index]: cells[index].strip() for index in range(len(headers))})
    return output


def table_value(row: dict[str, str], *keys: str) -> str:
    for key in keys:
        value = row.get(normalize_heading(key), "").strip()
        if value:
            return value
    return ""


def parse_answer_area(answer_area_md: str) -> list[tuple[str, str, str | None]]:
    parsed = []
    for row in parse_markdown_table(answer_area_md):
        prompt = table_value(row, "提示/位置", "prompt", "requirement", "陈述", "位置")
        recommended = table_value(row, "正确选择", "correct selection", "recommended selection", "正确判断")
        source = table_value(row, "源答案", "source selection") or None
        if prompt and recommended:
            parsed.append((prompt, recommended, source))
    return parsed


def truthy(value: str) -> bool:
    return value.strip().lower() in {"yes", "true", "1", "y", "是"}


def parse_drag_options(section_md: str) -> list[DragOption]:
    options: list[DragOption] = []
    for index, row in enumerate(parse_markdown_table(section_md), start=1):
        option_id = table_value(row, "option id", "id", "option")
        text = table_value(row, "text", "option text", "选项", "内容")
        group = table_value(row, "group", "option group", "分组") or None
        distractor = table_value(row, "distractor", "is distractor", "干扰项")
        if option_id and text:
            options.append(DragOption(option_id, text, group, truthy(distractor), index))
    return options


def parse_drag_slots(section_md: str) -> list[DragSlot]:
    slots: list[DragSlot] = []
    for index, row in enumerate(parse_markdown_table(section_md), start=1):
        slot_id = table_value(row, "slot id", "id", "slot")
        label = table_value(row, "label", "slot label", "提示/位置", "requirement")
        correct = table_value(row, "correct option id", "correct option", "正确选项id", "正确选择")
        if slot_id and label and correct:
            slots.append(DragSlot(slot_id, label, correct, index))
    return slots


def parse_ordered_slots(section_md: str) -> list[DragSlot]:
    slots: list[DragSlot] = []
    for index, row in enumerate(parse_markdown_table(section_md), start=1):
        slot_id = table_value(row, "slot id", "id", "slot")
        position = table_value(row, "position", "order", "sort") or str(index)
        label = table_value(row, "label", "slot label", "step", "提示/位置", "requirement") or f"Step {position}"
        correct = table_value(row, "correct option id", "correct option", "正确选项id", "正确选择")
        if slot_id and label and correct:
            slots.append(DragSlot(slot_id, f"{position}. {label}", correct, index))
    return slots


def parse_hotspot_options(section_md: str) -> list[HotspotOption]:
    options: list[HotspotOption] = []
    for index, row in enumerate(parse_markdown_table(section_md), start=1):
        option_id = table_value(row, "option id", "id", "option")
        text = table_value(row, "text", "option text", "选项", "内容")
        group = table_value(row, "group", "option group", "分组") or None
        sort_text = table_value(row, "sort", "sort order", "order")
        sort_order = int(sort_text) if sort_text.isdigit() else index
        if option_id and text:
            options.append(HotspotOption(option_id, text, group, sort_order))
    return options


def parse_hotspot_rows(section_md: str) -> list[HotspotRow]:
    rows: list[HotspotRow] = []
    for index, row in enumerate(parse_markdown_table(section_md), start=1):
        row_id = table_value(row, "row id", "id", "row")
        prompt = table_value(row, "prompt", "label", "提示/位置", "requirement")
        group = table_value(row, "option group", "group", "分组") or None
        correct = table_value(row, "correct option id", "correct option", "正确选项id", "正确选择")
        if row_id and prompt and correct:
            rows.append(HotspotRow(row_id, prompt, group, correct, index))
    return rows


def parse_interaction_options(section_md: str) -> list[InteractionOption]:
    options: list[InteractionOption] = []
    for index, row in enumerate(parse_markdown_table(section_md), start=1):
        option_id = table_value(row, "option id", "id", "option")
        text = table_value(row, "text", "option text", "选项", "内容")
        group = table_value(row, "group", "option group", "分组") or None
        distractor = table_value(row, "distractor", "is distractor", "干扰项")
        sort_text = table_value(row, "sort", "sort order", "order")
        sort_order = int(sort_text) if sort_text.isdigit() else index
        if option_id and text:
            options.append(InteractionOption(option_id, text, group, truthy(distractor), sort_order))
    return options


def parse_interaction_targets(section_md: str) -> list[InteractionTarget]:
    targets: list[InteractionTarget] = []
    for index, row in enumerate(parse_markdown_table(section_md), start=1):
        target_id = table_value(row, "target id", "row id", "slot id", "id", "target", "row", "slot")
        position_text = table_value(row, "position", "order", "sort")
        position = int(position_text) if position_text.isdigit() else index
        label = table_value(row, "label", "prompt", "slot label", "提示/位置", "requirement") or f"Step {position}"
        group = table_value(row, "option group", "group", "分组") or None
        correct = table_value(row, "correct option id", "correct option", "正确选项id", "正确选择")
        if target_id and label and correct:
            targets.append(InteractionTarget(target_id, position, label, group, correct))
    return targets


def interaction_options_to_drag(options: list[InteractionOption]) -> list[DragOption]:
    return [DragOption(option.id, option.text, option.group, option.is_distractor, option.sort_order) for option in options]


def interaction_options_to_hotspot(options: list[InteractionOption]) -> list[HotspotOption]:
    return [HotspotOption(option.id, option.text, option.group, option.sort_order) for option in options]


def interaction_targets_to_drag(targets: list[InteractionTarget], ordered: bool) -> list[DragSlot]:
    slots: list[DragSlot] = []
    for target in targets:
        label = f"{target.position}. {target.label}" if ordered else target.label
        slots.append(DragSlot(target.id, label, target.correct_option, target.position))
    return slots


def interaction_targets_to_hotspot(targets: list[InteractionTarget]) -> list[HotspotRow]:
    return [HotspotRow(target.id, target.label, target.option_group, target.correct_option, target.position) for target in targets]


def validate_drag(question: ParsedQuestion) -> list[str]:
    warnings: list[str] = []
    if question.question_type == "drag_drop" and (not question.drag_options or not question.drag_slots):
        warnings.append(f"{question.id}: drag_drop missing structured Drag Options or Drag Slots")
    if not question.drag_slots and not question.drag_options:
        return warnings
    option_ids = {option.id for option in question.drag_options}
    if len(option_ids) != len(question.drag_options):
        warnings.append(f"{question.id}: duplicate Drag Options ids")
    slot_ids = {slot.id for slot in question.drag_slots}
    if len(slot_ids) != len(question.drag_slots):
        warnings.append(f"{question.id}: duplicate Drag Slots ids")
    for slot in question.drag_slots:
        if slot.correct_option not in option_ids:
            warnings.append(f"{question.id}: slot {slot.id} references missing option {slot.correct_option}")
    return warnings


def validate_hotspot(question: ParsedQuestion) -> list[str]:
    warnings: list[str] = []
    if not question.hotspot_options and not question.hotspot_rows:
        return warnings
    option_ids = {option.id for option in question.hotspot_options}
    if len(option_ids) != len(question.hotspot_options):
        warnings.append(f"{question.id}: duplicate Hotspot Options ids")
    row_ids = {row.id for row in question.hotspot_rows}
    if len(row_ids) != len(question.hotspot_rows):
        warnings.append(f"{question.id}: duplicate Hotspot Rows ids")
    for row in question.hotspot_rows:
        if row.correct_option not in option_ids:
            warnings.append(f"{question.id}: hotspot row {row.id} references missing option {row.correct_option}")
    return warnings


def validate_interaction(question: ParsedQuestion) -> list[str]:
    warnings: list[str] = []
    if not question.interaction_options and not question.interaction_targets:
        return warnings
    option_ids = {option.id for option in question.interaction_options}
    if len(option_ids) != len(question.interaction_options):
        warnings.append(f"{question.id}: duplicate Interaction Options ids")
    target_ids = {target.id for target in question.interaction_targets}
    if len(target_ids) != len(question.interaction_targets):
        warnings.append(f"{question.id}: duplicate Interaction Targets ids")
    for target in question.interaction_targets:
        if target.correct_option not in option_ids:
            warnings.append(f"{question.id}: interaction target {target.id} references missing option {target.correct_option}")
    return warnings


def matched_group(match: re.Match[str] | None) -> str | None:
    if not match:
        return None
    for value in match.groups():
        if value:
            return value
    return None


def parse_batch(path: Path, root: Path, exam: str) -> ParsedBatch:
    text = path.read_text(encoding="utf-8")
    title = TITLE_RE.search(text).group(1) if TITLE_RE.search(text) else path.stem
    range_match = TITLE_QUESTIONS_RE.search(title) or TITLE_QUESTIONS_RE.search(path.name)
    batch_pages = BATCH_PAGES_RE.search(text)
    pdf_file = matched_group(PDF_RE.search(text))
    carryover = "\n".join(line.strip() for line in text.splitlines() if "carryover" in line.lower())
    batch_id = path.stem
    prefix = exam_id_prefix(exam)

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
        interaction = metas.get("interaction", "")
        interaction_options = parse_interaction_options(sections.get("interaction options", ""))
        interaction_targets = parse_interaction_targets(sections.get("interaction targets", ""))
        question_type = normalize_question_type(metas.get("type"))
        is_ordered = interaction.strip().lower().replace(" ", "_").replace("-", "_") == "ordered_list"
        drag_options = parse_drag_options(sections.get("drag options", ""))
        drag_slots = parse_drag_slots(sections.get("drag slots", "")) or parse_ordered_slots(sections.get("ordered slots", ""))
        hotspot_options = parse_hotspot_options(sections.get("hotspot options", ""))
        hotspot_rows = parse_hotspot_rows(sections.get("hotspot rows", ""))
        if interaction_options and question_type == "drag_drop" and not drag_options:
            drag_options = interaction_options_to_drag(interaction_options)
        if interaction_targets and question_type == "drag_drop" and not drag_slots:
            drag_slots = interaction_targets_to_drag(interaction_targets, is_ordered)
        if interaction_options and question_type == "hotspot" and not hotspot_options:
            hotspot_options = interaction_options_to_hotspot(interaction_options)
        if interaction_targets and question_type == "hotspot" and not hotspot_rows:
            hotspot_rows = interaction_targets_to_hotspot(interaction_targets)
        question = ParsedQuestion(
            id=f"{prefix}-q{q_number:04}",
            exam=exam,
            sequence_number=q_number,
            source_question_number=q_number,
            topic=metas.get("topic") or "Uncategorized",
            question_type=question_type,
            status=metas.get("status", "unknown"),
            source_pages=source_pages,
            page_from=page_from,
            page_to=page_to,
            question_text=sections.get("question", ""),
            options_md=sections.get("options", ""),
            answer_area_md=sections.get("answer area", sections.get("statements", "")),
            source_answer=sections.get("source answer", sections.get("correct answer", "")),
            recommended_answer=sections.get("my recommended answer", sections.get("correct answer", "")),
            chinese_judgement=sections.get("我的判断(中文)", sections.get("解析(中文)", sections.get("解析", ""))),
            reasoning=sections.get("reasoning", sections.get("key concept", "")),
            notes=sections.get("notes", ""),
            question_md=block,
            md_file=display_path(path),
            pdf_file=pdf_file,
            batch_id=batch_id,
            batch_order=index + 1,
            drag_options=drag_options,
            drag_slots=drag_slots,
            hotspot_options=hotspot_options,
            hotspot_rows=hotspot_rows,
            interaction_options=interaction_options,
            interaction_targets=interaction_targets,
        )
        questions.append(question)

    return ParsedBatch(
        id=batch_id,
        md_file=display_path(path),
        title=title,
        pdf_file=pdf_file,
        question_from=int(range_match.group(1)) if range_match else None,
        question_to=int(range_match.group(2)) if range_match else None,
        page_from=int(batch_pages.group(1)) if batch_pages else None,
        page_to=int(batch_pages.group(2)) if batch_pages else None,
        carryover=carryover,
        questions=questions,
    )


def create_schema(conn: sqlite3.Connection, reset: bool) -> None:
    if reset:
        conn.executescript("""
        DROP TABLE IF EXISTS drag_slots;
        DROP TABLE IF EXISTS drag_options;
        DROP TABLE IF EXISTS interaction_targets;
        DROP TABLE IF EXISTS interaction_options;
        DROP TABLE IF EXISTS hotspot_rows;
        DROP TABLE IF EXISTS hotspot_options;
        DROP TABLE IF EXISTS answer_areas;
        DROP TABLE IF EXISTS options;
        DROP TABLE IF EXISTS questions;
        DROP TABLE IF EXISTS markdown_batches;
        """)
    conn.executescript("""
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

    CREATE TABLE IF NOT EXISTS drag_options (
            id TEXT NOT NULL,
      question_id TEXT NOT NULL,
      option_text TEXT NOT NULL,
      option_group TEXT,
      is_distractor INTEGER NOT NULL DEFAULT 0,
      sort_order INTEGER NOT NULL,
            PRIMARY KEY(question_id, id),
      FOREIGN KEY(question_id) REFERENCES questions(id)
    );

    CREATE TABLE IF NOT EXISTS drag_slots (
            id TEXT NOT NULL,
      question_id TEXT NOT NULL,
      slot_label TEXT NOT NULL,
      correct_option TEXT NOT NULL,
      sort_order INTEGER NOT NULL,
            PRIMARY KEY(question_id, id),
      FOREIGN KEY(question_id) REFERENCES questions(id)
    );

        CREATE TABLE IF NOT EXISTS hotspot_options (
            id TEXT NOT NULL,
            question_id TEXT NOT NULL,
            option_text TEXT NOT NULL,
            option_group TEXT,
            sort_order INTEGER NOT NULL,
            PRIMARY KEY(question_id, id),
            FOREIGN KEY(question_id) REFERENCES questions(id)
        );

        CREATE TABLE IF NOT EXISTS hotspot_rows (
            id TEXT NOT NULL,
            question_id TEXT NOT NULL,
            prompt TEXT NOT NULL,
            option_group TEXT,
            correct_selection TEXT NOT NULL,
            sort_order INTEGER NOT NULL,
            PRIMARY KEY(question_id, id),
            FOREIGN KEY(question_id) REFERENCES questions(id)
        );

        CREATE TABLE IF NOT EXISTS interaction_options (
            id TEXT NOT NULL,
            question_id TEXT NOT NULL,
            option_text TEXT NOT NULL,
            option_group TEXT,
            is_distractor INTEGER NOT NULL DEFAULT 0,
            sort_order INTEGER NOT NULL,
            PRIMARY KEY(question_id, id),
            FOREIGN KEY(question_id) REFERENCES questions(id)
        );

        CREATE TABLE IF NOT EXISTS interaction_targets (
            id TEXT NOT NULL,
            question_id TEXT NOT NULL,
            position INTEGER NOT NULL,
            target_label TEXT NOT NULL,
            option_group TEXT,
            correct_option TEXT NOT NULL,
            PRIMARY KEY(question_id, id),
            FOREIGN KEY(question_id) REFERENCES questions(id)
        );

    CREATE INDEX IF NOT EXISTS idx_questions_pages ON questions(page_from, page_to);
    CREATE INDEX IF NOT EXISTS idx_questions_type_status ON questions(question_type, status);
    CREATE INDEX IF NOT EXISTS idx_options_question ON options(question_id);
    CREATE INDEX IF NOT EXISTS idx_answer_areas_question ON answer_areas(question_id);
    CREATE INDEX IF NOT EXISTS idx_drag_options_question ON drag_options(question_id);
    CREATE INDEX IF NOT EXISTS idx_drag_slots_question ON drag_slots(question_id);
    CREATE INDEX IF NOT EXISTS idx_hotspot_options_question ON hotspot_options(question_id);
    CREATE INDEX IF NOT EXISTS idx_hotspot_rows_question ON hotspot_rows(question_id);
    CREATE INDEX IF NOT EXISTS idx_interaction_options_question ON interaction_options(question_id);
    CREATE INDEX IF NOT EXISTS idx_interaction_targets_question ON interaction_targets(question_id);
    """)


def import_batches(conn: sqlite3.Connection, batches: list[ParsedBatch]) -> list[str]:
    warnings: list[str] = []
    now = datetime.now(timezone.utc).isoformat()
    for batch in batches:
        conn.execute(
            """
            INSERT OR REPLACE INTO markdown_batches
            (id, md_file, title, pdf_file, question_from, question_to, page_from, page_to, carryover, question_count, imported_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (batch.id, batch.md_file, batch.title, batch.pdf_file, batch.question_from, batch.question_to, batch.page_from, batch.page_to, batch.carryover, len(batch.questions), now),
        )
        for question in batch.questions:
            warnings.extend(validate_drag(question))
            warnings.extend(validate_hotspot(question))
            warnings.extend(validate_interaction(question))
            conn.execute("DELETE FROM options WHERE question_id = ?", (question.id,))
            conn.execute("DELETE FROM answer_areas WHERE question_id = ?", (question.id,))
            conn.execute("DELETE FROM drag_options WHERE question_id = ?", (question.id,))
            conn.execute("DELETE FROM drag_slots WHERE question_id = ?", (question.id,))
            conn.execute("DELETE FROM hotspot_options WHERE question_id = ?", (question.id,))
            conn.execute("DELETE FROM hotspot_rows WHERE question_id = ?", (question.id,))
            conn.execute("DELETE FROM interaction_options WHERE question_id = ?", (question.id,))
            conn.execute("DELETE FROM interaction_targets WHERE question_id = ?", (question.id,))
            conn.execute(
                """
                INSERT OR REPLACE INTO questions
                (id, exam, sequence_number, source_question_number, topic, question_type, status, source_pages, page_from, page_to,
                 question_text, options_md, answer_area_md, source_answer, recommended_answer, chinese_judgement, reasoning, notes,
                 question_md, md_file, pdf_file, batch_id, batch_order, imported_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (question.id, question.exam, question.sequence_number, question.source_question_number, question.topic, question.question_type,
                 question.status, question.source_pages, question.page_from, question.page_to, question.question_text, question.options_md,
                 question.answer_area_md, question.source_answer, question.recommended_answer, question.chinese_judgement, question.reasoning,
                 question.notes, question.question_md, question.md_file, question.pdf_file, question.batch_id, question.batch_order, now),
            )
            for order, (key, text) in enumerate(parse_options(question.options_md), start=1):
                conn.execute("INSERT INTO options (question_id, option_key, option_text, sort_order) VALUES (?, ?, ?, ?)", (question.id, key, text, order))
            for order, (prompt, recommended, source) in enumerate(parse_answer_area(question.answer_area_md), start=1):
                conn.execute("INSERT INTO answer_areas (question_id, prompt, source_selection, recommended_selection, sort_order) VALUES (?, ?, ?, ?, ?)", (question.id, prompt, source, recommended, order))
            for option in question.drag_options:
                conn.execute("INSERT INTO drag_options (id, question_id, option_text, option_group, is_distractor, sort_order) VALUES (?, ?, ?, ?, ?, ?)", (option.id, question.id, option.text, option.group, int(option.is_distractor), option.sort_order))
            for slot in question.drag_slots:
                conn.execute("INSERT INTO drag_slots (id, question_id, slot_label, correct_option, sort_order) VALUES (?, ?, ?, ?, ?)", (slot.id, question.id, slot.label, slot.correct_option, slot.sort_order))
            for option in question.hotspot_options:
                conn.execute("INSERT INTO hotspot_options (id, question_id, option_text, option_group, sort_order) VALUES (?, ?, ?, ?, ?)", (option.id, question.id, option.text, option.group, option.sort_order))
            for row in question.hotspot_rows:
                conn.execute("INSERT INTO hotspot_rows (id, question_id, prompt, option_group, correct_selection, sort_order) VALUES (?, ?, ?, ?, ?, ?)", (row.id, question.id, row.prompt, row.option_group, row.correct_option, row.sort_order))
            for option in question.interaction_options:
                conn.execute("INSERT INTO interaction_options (id, question_id, option_text, option_group, is_distractor, sort_order) VALUES (?, ?, ?, ?, ?, ?)", (option.id, question.id, option.text, option.group, int(option.is_distractor), option.sort_order))
            for target in question.interaction_targets:
                conn.execute("INSERT INTO interaction_targets (id, question_id, position, target_label, option_group, correct_option) VALUES (?, ?, ?, ?, ?, ?)", (target.id, question.id, target.position, target.label, target.option_group, target.correct_option))
    return warnings


def main() -> None:
    args = parse_args()
    root = Path(args.input)
    if not root.exists():
        raise SystemExit(f"Input folder does not exist: {root}")
    exam = args.exam.upper()
    db_path = Path(args.db) if args.db else Path("question-banks") / f"{exam}.sqlite"
    files = sorted(root.rglob(args.pattern))
    if not files:
        raise SystemExit(f"No Markdown files matched {args.pattern} under {root}")

    batches = [parse_batch(path, root, exam) for path in files]
    db_path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(str(db_path))
    try:
        create_schema(conn, args.reset)
        warnings = import_batches(conn, batches)
        conn.commit()
    finally:
        conn.close()

    total = sum(len(batch.questions) for batch in batches)
    print(f"Imported {total} question(s) from {len(batches)} file(s) into {db_path}")
    for warning in warnings:
        print(f"Warning: {warning}")


if __name__ == "__main__":
    main()
