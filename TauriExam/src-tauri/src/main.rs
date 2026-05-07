#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use uuid::Uuid;

#[derive(Debug, Serialize)]
struct BankInfo {
    id: String,
    exam_code: String,
    name: String,
    db_path: String,
    pdf_path: String,
    question_count: i64,
}

#[derive(Debug, Clone)]
struct BankEntry {
    id: String,
    exam_code: String,
    name: String,
    db_path: PathBuf,
    pdf_path: Option<PathBuf>,
    cache_dir: PathBuf,
    question_count: i64,
}

#[derive(Debug, Serialize)]
struct QuestionSummary {
    id: String,
    sequence_number: i64,
    question_type: String,
    status: String,
    page_from: Option<i64>,
    page_to: Option<i64>,
    preview: String,
    recommended_answer: String,
}

#[derive(Debug, Serialize)]
struct OptionRow {
    option_key: String,
    option_text: String,
    sort_order: i64,
}

#[derive(Debug, Serialize)]
struct AnswerAreaRow {
    prompt: String,
    source_selection: Option<String>,
    recommended_selection: String,
    sort_order: i64,
}

#[derive(Debug, Serialize)]
struct QuestionDetail {
    id: String,
    sequence_number: i64,
    source_question_number: Option<i64>,
    topic: Option<String>,
    question_type: String,
    status: String,
    source_pages: Option<String>,
    page_from: Option<i64>,
    page_to: Option<i64>,
    question_text: String,
    options_md: Option<String>,
    answer_area_md: Option<String>,
    source_answer: Option<String>,
    recommended_answer: Option<String>,
    chinese_judgement: Option<String>,
    reasoning: Option<String>,
    notes: Option<String>,
    question_md: String,
    md_file: String,
    pdf_file: Option<String>,
    options: Vec<OptionRow>,
    answer_areas: Vec<AnswerAreaRow>,
}

#[derive(Debug, Serialize)]
struct PageImage {
    page: i64,
    path: String,
    data_url: String,
}

#[derive(Debug, Deserialize)]
struct SaveExamAnswerInput {
    question_id: String,
    sequence_number: i64,
    user_answer: String,
    correct_answer: String,
    recommended_answer: String,
    is_correct: Option<bool>,
    duration_seconds: i64,
}

#[derive(Debug, Deserialize)]
struct SaveExamInput {
    bank_id: String,
    title: String,
    mode: String,
    duration_seconds: i64,
    answers: Vec<SaveExamAnswerInput>,
}

#[derive(Debug, Serialize)]
struct SavedExam {
    id: String,
    total_questions: i64,
    correct_count: i64,
    wrong_count: i64,
}

#[derive(Debug, Serialize)]
struct ExamSessionSummary {
    id: String,
    bank_id: String,
    title: String,
    mode: String,
    started_at: String,
    finished_at: String,
    duration_seconds: i64,
    total_questions: i64,
    correct_count: i64,
    wrong_count: i64,
}

#[derive(Debug, Serialize)]
struct ExamAnswerDetail {
    id: String,
    session_id: String,
    bank_id: String,
    question_id: String,
    sequence_number: i64,
    user_answer: String,
    correct_answer: Option<String>,
    recommended_answer: Option<String>,
    is_correct: Option<bool>,
    duration_seconds: i64,
    created_at: String,
}

fn workspace_root() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|err| err.to_string())?;
    for dir in cwd.ancestors() {
        if dir.join("output").exists() && dir.join("TauriExam").exists() {
            return Ok(dir.to_path_buf());
        }
        if dir.file_name().and_then(|name| name.to_str()) == Some("TauriExam") {
            if let Some(parent) = dir.parent() {
                return Ok(parent.to_path_buf());
            }
        }
    }
    Ok(cwd.parent().unwrap_or(&cwd).to_path_buf())
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|path| path.parent().map(Path::to_path_buf))
}

fn question_bank_roots() -> Result<Vec<PathBuf>, String> {
    let mut roots = Vec::new();
    if let Some(dir) = exe_dir() {
        roots.push(dir.join("question-banks"));
    }
    roots.push(std::env::current_dir().map_err(|err| err.to_string())?.join("question-banks"));
    roots.push(workspace_root()?.join("question-banks"));
    roots.dedup();
    Ok(roots)
}

fn normalize_bank_id(value: &str) -> String {
    let mut id = String::new();
    let mut previous_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            id.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            id.push('-');
            previous_dash = true;
        }
    }
    id.trim_matches('-').to_string()
}

fn sqlite_has_questions(path: &Path) -> bool {
    let Ok(conn) = Connection::open(path) else {
        return false;
    };
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'questions'",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .unwrap_or(false)
}

fn discover_banks() -> Result<Vec<BankEntry>, String> {
    let mut banks = Vec::new();
    for root in question_bank_roots()? {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries {
            let entry = entry.map_err(|err| err.to_string())?;
            let db_path = entry.path();
            if db_path.extension().and_then(|ext| ext.to_str()).map(|ext| !ext.eq_ignore_ascii_case("sqlite")).unwrap_or(true) {
                continue;
            }
            if !sqlite_has_questions(&db_path) {
                continue;
            }
            let Some(stem) = db_path.file_stem().and_then(|name| name.to_str()) else {
                continue;
            };
            let id = normalize_bank_id(stem);
            if banks.iter().any(|bank: &BankEntry| bank.id == id) {
                continue;
            }
            let pdf_path = db_path.with_extension("pdf");
            let question_count = Connection::open(&db_path)
                .ok()
                .and_then(|conn| conn.query_row("SELECT COUNT(*) FROM questions", [], |row| row.get(0)).ok())
                .unwrap_or(0);
            banks.push(BankEntry {
                id,
                exam_code: stem.to_string(),
                name: format!("{stem} 题库"),
                db_path: db_path.clone(),
                pdf_path: pdf_path.exists().then_some(pdf_path),
                cache_dir: root.join(".page-cache").join(normalize_bank_id(stem)),
                question_count,
            });
        }
    }
    banks.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(banks)
}

fn find_bank(bank_id: &str) -> Result<BankEntry, String> {
    discover_banks()?
        .into_iter()
        .find(|bank| bank.id == bank_id)
        .ok_or_else(|| format!("Unknown question bank: {bank_id}. Put <name>.sqlite and optional <name>.pdf into question-banks."))
}

fn bank_db_path(bank_id: &str) -> Result<PathBuf, String> {
    Ok(find_bank(bank_id)?.db_path)
}

fn app_db_path() -> Result<PathBuf, String> {
    Ok(workspace_root()?.join("output/exam-tool/exam-tool.sqlite"))
}

fn open_bank(bank_id: &str) -> Result<Connection, String> {
    let path = bank_db_path(bank_id)?;
    Connection::open(&path).map_err(|err| format!("Failed to open {}: {err}", path.display()))
}

fn open_app_db() -> Result<Connection, String> {
    let path = app_db_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let conn = Connection::open(&path).map_err(|err| format!("Failed to open {}: {err}", path.display()))?;
    init_app_schema(&conn)?;
    Ok(conn)
}

fn init_app_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS question_banks (
          id TEXT PRIMARY KEY,
          exam_code TEXT NOT NULL,
          name TEXT NOT NULL,
          db_path TEXT NOT NULL,
          pdf_path TEXT,
          kind TEXT NOT NULL DEFAULT 'sqlite',
          enabled INTEGER NOT NULL DEFAULT 1,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS exam_sessions (
          id TEXT PRIMARY KEY,
          bank_id TEXT NOT NULL,
          title TEXT NOT NULL,
          mode TEXT NOT NULL,
          started_at TEXT NOT NULL,
          finished_at TEXT NOT NULL,
          duration_seconds INTEGER NOT NULL,
          total_questions INTEGER NOT NULL,
          correct_count INTEGER NOT NULL,
          wrong_count INTEGER NOT NULL,
          created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS exam_answers (
          id TEXT PRIMARY KEY,
          session_id TEXT NOT NULL,
          bank_id TEXT NOT NULL,
          question_id TEXT NOT NULL,
          sequence_number INTEGER NOT NULL,
          user_answer TEXT NOT NULL,
          correct_answer TEXT,
          recommended_answer TEXT,
          is_correct INTEGER,
          duration_seconds INTEGER NOT NULL,
          created_at TEXT NOT NULL,
          FOREIGN KEY(session_id) REFERENCES exam_sessions(id)
        );

        CREATE TABLE IF NOT EXISTS question_flags (
          id TEXT PRIMARY KEY,
          bank_id TEXT NOT NULL,
          question_id TEXT NOT NULL,
          flag_type TEXT NOT NULL,
          note TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE(bank_id, question_id, flag_type)
        );

        CREATE TABLE IF NOT EXISTS translations (
          id TEXT PRIMARY KEY,
          bank_id TEXT NOT NULL,
          question_id TEXT NOT NULL,
          field_name TEXT NOT NULL,
          source_hash TEXT NOT NULL,
          language TEXT NOT NULL,
          translated_text TEXT NOT NULL,
          provider TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          UNIQUE(bank_id, question_id, field_name, language, source_hash)
        );
        "#,
    )
    .map_err(|err| err.to_string())
}

#[tauri::command]
fn list_banks() -> Result<Vec<BankInfo>, String> {
    discover_banks().map(|banks| {
        banks
            .into_iter()
            .map(|bank| BankInfo {
                id: bank.id,
                exam_code: bank.exam_code,
                name: bank.name,
                db_path: bank.db_path.display().to_string(),
                pdf_path: bank.pdf_path.map(|path| path.display().to_string()).unwrap_or_default(),
                question_count: bank.question_count,
            })
            .collect()
    })
}

#[tauri::command]
fn list_questions(bank_id: String) -> Result<Vec<QuestionSummary>, String> {
    let conn = open_bank(&bank_id)?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, sequence_number, question_type, status, page_from, page_to,
                   substr(replace(question_text, char(10), ' '), 1, 180) AS preview,
                   coalesce(recommended_answer, '')
            FROM questions
            ORDER BY sequence_number
            "#,
        )
        .map_err(|err| err.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(QuestionSummary {
                id: row.get(0)?,
                sequence_number: row.get(1)?,
                question_type: row.get(2)?,
                status: row.get(3)?,
                page_from: row.get(4)?,
                page_to: row.get(5)?,
                preview: row.get(6)?,
                recommended_answer: row.get(7)?,
            })
        })
        .map_err(|err| err.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|err| err.to_string())
}

#[tauri::command]
fn get_question(bank_id: String, question_id: String) -> Result<QuestionDetail, String> {
    let conn = open_bank(&bank_id)?;
    let mut question = conn
        .query_row(
            r#"
            SELECT id, sequence_number, source_question_number, topic, question_type, status,
                   source_pages, page_from, page_to, question_text, options_md, answer_area_md,
                   source_answer, recommended_answer, chinese_judgement, reasoning, notes,
                   question_md, md_file, pdf_file
            FROM questions
            WHERE id = ?1
            "#,
            params![question_id],
            |row| {
                Ok(QuestionDetail {
                    id: row.get(0)?,
                    sequence_number: row.get(1)?,
                    source_question_number: row.get(2)?,
                    topic: row.get(3)?,
                    question_type: row.get(4)?,
                    status: row.get(5)?,
                    source_pages: row.get(6)?,
                    page_from: row.get(7)?,
                    page_to: row.get(8)?,
                    question_text: row.get(9)?,
                    options_md: row.get(10)?,
                    answer_area_md: row.get(11)?,
                    source_answer: row.get(12)?,
                    recommended_answer: row.get(13)?,
                    chinese_judgement: row.get(14)?,
                    reasoning: row.get(15)?,
                    notes: row.get(16)?,
                    question_md: row.get(17)?,
                    md_file: row.get(18)?,
                    pdf_file: row.get(19)?,
                    options: Vec::new(),
                    answer_areas: Vec::new(),
                })
            },
        )
        .map_err(|err| err.to_string())?;

    let mut options_stmt = conn
        .prepare("SELECT option_key, option_text, sort_order FROM options WHERE question_id = ?1 ORDER BY sort_order")
        .map_err(|err| err.to_string())?;
    question.options = options_stmt
        .query_map(params![question.id.clone()], |row| {
            Ok(OptionRow {
                option_key: row.get(0)?,
                option_text: row.get(1)?,
                sort_order: row.get(2)?,
            })
        })
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;

    let mut answer_stmt = conn
        .prepare(
            "SELECT prompt, source_selection, recommended_selection, sort_order FROM answer_areas WHERE question_id = ?1 ORDER BY sort_order",
        )
        .map_err(|err| err.to_string())?;
    question.answer_areas = answer_stmt
        .query_map(params![question.id.clone()], |row| {
            Ok(AnswerAreaRow {
                prompt: row.get(0)?,
                source_selection: row.get(1)?,
                recommended_selection: row.get(2)?,
                sort_order: row.get(3)?,
            })
        })
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;

    Ok(question)
}

#[tauri::command]
fn get_source_pages(bank_id: String, question_id: String) -> Result<Vec<PageImage>, String> {
    let bank = find_bank(&bank_id)?;
    let question = get_question(bank_id.clone(), question_id)?;
    let from = question.page_from.ok_or("Question has no page_from")?;
    let to = question.page_to.ok_or("Question has no page_to")?;
    let mut pages = Vec::new();

    for page in from..=to {
        let path = ensure_page_image(&bank, page)?;
        if path.exists() {
            let bytes = fs::read(&path).map_err(|err| err.to_string())?;
            let encoded = general_purpose::STANDARD.encode(bytes);
            pages.push(PageImage {
                page,
                path: path.display().to_string(),
                data_url: format!("data:image/png;base64,{encoded}"),
            });
        }
    }

    Ok(pages)
}

fn ensure_page_image(bank: &BankEntry, page: i64) -> Result<PathBuf, String> {
    let cache_path = bank.cache_dir.join(format!("page-{page:03}.png"));
    if cache_path.exists() {
        return Ok(cache_path);
    }
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    if let Some(pdf_path) = &bank.pdf_path {
        if render_pdf_page(pdf_path, page, &cache_path).is_ok() && cache_path.exists() {
            return Ok(cache_path);
        }
    }
    if let Some(path) = find_page_image(&workspace_root()?.join("output/vision-pages"), page)? {
        fs::copy(&path, &cache_path).map_err(|err| err.to_string())?;
        return Ok(cache_path);
    }
    if bank.pdf_path.is_none() {
        return Err(format!("No matching PDF found for bank {}. Put {}.pdf next to the SQLite file.", bank.name, bank.exam_code));
    }
    Err(format!("Failed to render PDF page {page}. Install Python with PyMuPDF for on-demand rendering, or pre-cache the page image."))
}

fn render_pdf_page(pdf_path: &Path, page: i64, output_path: &Path) -> Result<(), String> {
    let script = r#"
import sys
from pathlib import Path
import fitz
pdf = Path(sys.argv[1])
page = int(sys.argv[2])
out = Path(sys.argv[3])
out.parent.mkdir(parents=True, exist_ok=True)
doc = fitz.open(str(pdf))
pix = doc.load_page(page - 1).get_pixmap(matrix=fitz.Matrix(1.6, 1.6), alpha=False)
pix.save(str(out))
"#;
    let mut errors = Vec::new();
    for program in ["py", "python", "python3"] {
        let mut command = Command::new(program);
        if program == "py" {
            command.arg("-3");
        }
        command.args(["-c", script, &pdf_path.display().to_string(), &page.to_string(), &output_path.display().to_string()]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        match command.output() {
            Ok(output) if output.status.success() => return Ok(()),
            Ok(output) => errors.push(format!("{program}: {}", String::from_utf8_lossy(&output.stderr).trim())),
            Err(err) => errors.push(format!("{program}: {err}")),
        }
    }
    Err(errors.join("; "))
}

fn find_page_image(root: &Path, page: i64) -> Result<Option<PathBuf>, String> {
    let file_name = format!("page-{page:03}.png");
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries {
            let entry = entry.map_err(|err| err.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().and_then(|name| name.to_str()) == Some(file_name.as_str()) {
                return Ok(Some(path));
            }
        }
    }
    Ok(None)
}

#[tauri::command]
fn save_exam_result(input: SaveExamInput) -> Result<SavedExam, String> {
    let conn = open_app_db()?;
    let now = Utc::now().to_rfc3339();
    let session_id = Uuid::new_v4().to_string();
    let total = input.answers.len() as i64;
    let correct = input.answers.iter().filter(|answer| answer.is_correct == Some(true)).count() as i64;
    let wrong = input.answers.iter().filter(|answer| answer.is_correct == Some(false)).count() as i64;

    conn.execute(
        r#"
        INSERT INTO exam_sessions
        (id, bank_id, title, mode, started_at, finished_at, duration_seconds, total_questions, correct_count, wrong_count, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, ?7, ?8, ?9, ?5)
        "#,
        params![session_id, input.bank_id, input.title, input.mode, now, input.duration_seconds, total, correct, wrong],
    )
    .map_err(|err| err.to_string())?;

    for answer in input.answers {
        conn.execute(
            r#"
            INSERT INTO exam_answers
            (id, session_id, bank_id, question_id, sequence_number, user_answer, correct_answer, recommended_answer, is_correct, duration_seconds, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                Uuid::new_v4().to_string(),
                session_id,
                input.bank_id,
                answer.question_id,
                answer.sequence_number,
                answer.user_answer,
                answer.correct_answer,
                answer.recommended_answer,
                answer.is_correct.map(|value| if value { 1 } else { 0 }),
                answer.duration_seconds,
                now
            ],
        )
        .map_err(|err| err.to_string())?;
    }

    Ok(SavedExam {
        id: session_id,
        total_questions: total,
        correct_count: correct,
        wrong_count: wrong,
    })
}

#[tauri::command]
fn list_exam_sessions() -> Result<Vec<ExamSessionSummary>, String> {
    let conn = open_app_db()?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, bank_id, title, mode, started_at, finished_at, duration_seconds, total_questions, correct_count, wrong_count
            FROM exam_sessions
            ORDER BY created_at DESC
            "#,
        )
        .map_err(|err| err.to_string())?;

    let sessions = stmt.query_map([], |row| {
        Ok(ExamSessionSummary {
            id: row.get(0)?,
            bank_id: row.get(1)?,
            title: row.get(2)?,
            mode: row.get(3)?,
            started_at: row.get(4)?,
            finished_at: row.get(5)?,
            duration_seconds: row.get(6)?,
            total_questions: row.get(7)?,
            correct_count: row.get(8)?,
            wrong_count: row.get(9)?,
        })
    })
    .map_err(|err| err.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|err| err.to_string())?;

    Ok(sessions)
}

#[tauri::command]
fn list_exam_answers(session_id: String) -> Result<Vec<ExamAnswerDetail>, String> {
    let conn = open_app_db()?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, session_id, bank_id, question_id, sequence_number, user_answer,
                   correct_answer, recommended_answer, is_correct, duration_seconds, created_at
            FROM exam_answers
            WHERE session_id = ?1
            ORDER BY rowid
            "#,
        )
        .map_err(|err| err.to_string())?;

    let answers = stmt.query_map(params![session_id], |row| {
        let is_correct: Option<i64> = row.get(8)?;
        Ok(ExamAnswerDetail {
            id: row.get(0)?,
            session_id: row.get(1)?,
            bank_id: row.get(2)?,
            question_id: row.get(3)?,
            sequence_number: row.get(4)?,
            user_answer: row.get(5)?,
            correct_answer: row.get(6)?,
            recommended_answer: row.get(7)?,
            is_correct: is_correct.map(|value| value != 0),
            duration_seconds: row.get(9)?,
            created_at: row.get(10)?,
        })
    })
    .map_err(|err| err.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|err| err.to_string())?;

    Ok(answers)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            list_banks,
            list_questions,
            get_question,
            get_source_pages,
            save_exam_result,
            list_exam_sessions,
            list_exam_answers
        ])
        .run(tauri::generate_context!())
        .expect("error while running TauriExam");
}
