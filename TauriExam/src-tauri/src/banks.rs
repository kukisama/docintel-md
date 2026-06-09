//! 题库、题目、考试记录、错题/收藏标记与 PDF 原文页渲染相关的 Tauri 命令。

use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use pdfium_render::prelude::*;
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

use crate::models::*;
use crate::storage::{
    app_data_dir, app_db_path, discover_banks, ensure_app_dirs, exe_dir, find_bank, open_app_db, open_bank,
    page_cache_dir, question_banks_dir, translation_db_path_for_bank, workspace_root,
};

static PDF_RENDER_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[tauri::command]
pub(crate) fn get_app_paths() -> Result<AppPaths, String> {
    ensure_app_dirs()?;
    Ok(AppPaths {
        data_dir: app_data_dir()?.display().to_string(),
        app_db_path: app_db_path()?.display().to_string(),
        question_banks_dir: question_banks_dir()?.display().to_string(),
        page_cache_dir: page_cache_dir()?.display().to_string(),
    })
}

#[tauri::command]
pub(crate) fn refresh_banks() -> Result<Vec<BankInfo>, String> {
    list_banks()
}

#[tauri::command]
pub(crate) fn open_data_dir() -> Result<(), String> {
    ensure_app_dirs()?;
    open_dir(&app_data_dir()?)
}

#[tauri::command]
pub(crate) fn open_question_banks_dir() -> Result<(), String> {
    ensure_app_dirs()?;
    open_dir(&question_banks_dir()?)
}

#[tauri::command]
pub(crate) fn check_bank_health(bank_id: String) -> Result<BankHealth, String> {
    let bank = find_bank(&bank_id)?;
    let conn = Connection::open(&bank.db_path).map_err(|err| err.to_string())?;
    let question_count = conn.query_row("SELECT COUNT(*) FROM questions", [], |row| row.get(0)).unwrap_or(0);
    let empty_question_count = conn
        .query_row("SELECT COUNT(*) FROM questions WHERE trim(coalesce(question_text, '')) = ''", [], |row| row.get(0))
        .unwrap_or(0);
    let empty_answer_count = conn
        .query_row(
            "SELECT COUNT(*) FROM questions WHERE trim(coalesce(source_answer, '')) = '' AND trim(coalesce(recommended_answer, '')) = ''",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let missing_page_count = conn
        .query_row("SELECT COUNT(*) FROM questions WHERE page_from IS NULL OR page_to IS NULL", [], |row| row.get(0))
        .unwrap_or(0);
    let max_question_page = conn.query_row("SELECT max(page_to) FROM questions", [], |row| row.get(0)).unwrap_or(None);

    // Check translation database
    let trans_path = translation_db_path_for_bank(&bank)?;
    let trans_path_str = trans_path.display().to_string();
    let translation_db_exists = trans_path.exists();
    let translated_count: i64 = if translation_db_exists {
        if let Ok(tconn) = Connection::open(&trans_path) {
            tconn
                .query_row(
                    "SELECT COUNT(DISTINCT question_id) FROM translation_segments WHERE bank_id = ?1",
                    [&bank.id],
                    |row| row.get(0),
                )
                .unwrap_or(0)
        } else {
            0
        }
    } else {
        0
    };

    let mut warnings = Vec::new();
    if bank.pdf_path.is_none() {
        warnings.push("未找到同名 PDF，可以继续刷题，但无法加载原文页。".to_string());
    }
    if empty_question_count > 0 {
        warnings.push(format!("发现 {empty_question_count} 道题题干为空。"));
    }
    if empty_answer_count > 0 {
        warnings.push(format!("发现 {empty_answer_count} 道题缺少源答案和推荐答案。"));
    }
    if missing_page_count > 0 {
        warnings.push(format!("发现 {missing_page_count} 道题缺少 PDF 页码。"));
    }
    if warnings.is_empty() {
        warnings.push("题库基础检查通过。".to_string());
    }
    let remaining = question_count - translated_count;
    if translation_db_exists {
        if remaining > 0 {
            warnings.push(format!("翻译库还差 {remaining} 题未翻译，建议前往「翻译服务」执行批量翻译。"));
        } else {
            warnings.push("翻译库已完整覆盖所有题目。".to_string());
        }
    } else {
        warnings.push("尚未创建翻译库，可前往「翻译服务」执行批量翻译。".to_string());
    }

    Ok(BankHealth {
        bank_id,
        sqlite_ok: true,
        pdf_found: bank.pdf_path.is_some(),
        question_count,
        empty_question_count,
        empty_answer_count,
        missing_page_count,
        max_question_page,
        pdf_page_count: None,
        translation_db_path: trans_path_str,
        translation_db_exists,
        translated_count,
        warnings,
    })
}

fn open_dir(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        Command::new("explorer").arg(path).spawn().map_err(|err| err.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn().map_err(|err| err.to_string())?;
        return Ok(());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(path).spawn().map_err(|err| err.to_string())?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err("Unsupported platform for opening directories.".to_string())
}

#[tauri::command]
pub(crate) fn list_banks() -> Result<Vec<BankInfo>, String> {
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
pub(crate) fn list_questions(bank_id: String) -> Result<Vec<QuestionSummary>, String> {
    let conn = open_bank(&bank_id)?;
    let mut stmt = conn
        .prepare(
            r#"
                 SELECT id, sequence_number, topic, question_type, status, page_from, page_to,
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
                topic: row.get(2)?,
                question_type: row.get(3)?,
                status: row.get(4)?,
                page_from: row.get(5)?,
                page_to: row.get(6)?,
                preview: row.get(7)?,
                recommended_answer: row.get(8)?,
            })
        })
        .map_err(|err| err.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|err| err.to_string())
}

#[tauri::command]
pub(crate) fn get_question(bank_id: String, question_id: String) -> Result<QuestionDetail, String> {
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
pub(crate) fn get_source_pages(bank_id: String, question_id: String) -> Result<Vec<PageImage>, String> {
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
    Err(format!("Failed to render PDF page {page}. PDFium runtime was not available or the page could not be rendered."))
}

fn render_pdf_page(pdf_path: &Path, page: i64, output_path: &Path) -> Result<(), String> {
    let _guard = PDF_RENDER_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "PDF 渲染锁已损坏，请重启应用后再试。".to_string())?;
    if output_path.exists() {
        return Ok(());
    }
    render_pdf_page_native(pdf_path, page, output_path)
}

fn render_pdf_page_native(pdf_path: &Path, page: i64, output_path: &Path) -> Result<(), String> {
    if page <= 0 {
        return Err("PDF page number must start from 1.".to_string());
    }
    let pdfium = bind_pdfium_safely()?;
    let document = pdfium.load_pdf_from_file(pdf_path, None).map_err(|err| err.to_string())?;
    let page_index = i32::try_from(page - 1).map_err(|_| "PDF page index is too large.".to_string())?;
    let pdf_page = document.pages().get(page_index).map_err(|err| err.to_string())?;
    let render_config = PdfRenderConfig::new().set_target_width(1600).set_maximum_height(2400);
    let image = pdf_page
        .render_with_config(&render_config)
        .map_err(|err| err.to_string())?
        .as_image()
        .map_err(|err| err.to_string())?
        .into_rgb8();
    image.save(output_path).map_err(|err| err.to_string())
}

fn bind_pdfium_safely() -> Result<Pdfium, String> {
    let lib_name = Pdfium::pdfium_platform_library_name();
    let mut candidates = Vec::new();
    if let Some(dir) = exe_dir() {
        // 1) Same dir as the executable (Windows portable layout, dev cargo target).
        candidates.push(Pdfium::pdfium_platform_library_name_at_path(&dir));
        // 2) <exe_dir>/resources/<lib>  (Windows MSI/NSIS install layout)
        candidates.push(dir.join("resources").join(&lib_name));
        // 3) macOS .app bundle: exe sits in <App>.app/Contents/MacOS,
        //    resources land at <App>.app/Contents/Resources/resources/<lib>
        if let Some(parent) = dir.parent() {
            candidates.push(parent.join("Resources").join("resources").join(&lib_name));
            candidates.push(parent.join("Resources").join(&lib_name));
            // 4) Fallback for any layout that uses lowercase `resources`.
            candidates.push(parent.join("resources").join(&lib_name));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(Pdfium::pdfium_platform_library_name_at_path(&cwd));
        candidates.push(cwd.join("resources").join(&lib_name));
        candidates.push(cwd.join("src-tauri").join("resources").join(&lib_name));
    }

    let mut errors = Vec::new();
    for candidate in candidates {
        match Pdfium::bind_to_library(&candidate) {
            Ok(bindings) => return Ok(Pdfium::new(bindings)),
            Err(PdfiumError::PdfiumLibraryBindingsAlreadyInitialized) => return Ok(Pdfium::default()),
            Err(err) => errors.push(format!("{}: {err}", candidate.display())),
        }
    }

    match Pdfium::bind_to_system_library() {
        Ok(bindings) => Ok(Pdfium::new(bindings)),
        Err(PdfiumError::PdfiumLibraryBindingsAlreadyInitialized) => Ok(Pdfium::default()),
        Err(err) => {
            errors.push(format!("system library: {err}"));
            Err(errors.join("; "))
        }
    }
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
pub(crate) fn save_exam_result(input: SaveExamInput) -> Result<SavedExam, String> {
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
pub(crate) fn list_exam_sessions() -> Result<Vec<ExamSessionSummary>, String> {
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
pub(crate) fn list_exam_answers(session_id: String) -> Result<Vec<ExamAnswerDetail>, String> {
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

#[tauri::command]
pub(crate) fn get_question_practice_stats(bank_id: String, question_ids: Vec<String>) -> Result<Vec<QuestionPracticeStats>, String> {
    let conn = open_app_db()?;
    let requested: HashSet<String> = question_ids.into_iter().collect();
    if requested.is_empty() {
        return Ok(Vec::new());
    }

    let mut latest: HashMap<String, (Option<bool>, String)> = HashMap::new();
    let mut latest_stmt = conn
        .prepare(
            r#"
            SELECT question_id, is_correct, created_at
            FROM exam_answers
            WHERE bank_id = ?1
            ORDER BY created_at DESC, rowid DESC
            "#,
        )
        .map_err(|err| err.to_string())?;
    let latest_rows = latest_stmt
        .query_map(params![bank_id.clone()], |row| {
            let question_id: String = row.get(0)?;
            let is_correct: Option<i64> = row.get(1)?;
            let created_at: String = row.get(2)?;
            Ok((question_id, is_correct.map(|value| value != 0), created_at))
        })
        .map_err(|err| err.to_string())?;
    for row in latest_rows {
        let (question_id, is_correct, created_at) = row.map_err(|err| err.to_string())?;
        if requested.contains(&question_id) {
            latest.entry(question_id).or_insert((is_correct, created_at));
        }
    }

    let mut stats_stmt = conn
        .prepare(
            r#"
            SELECT question_id,
                   COUNT(*) AS attempt_count,
                   SUM(CASE WHEN is_correct = 0 THEN 1 ELSE 0 END) AS wrong_count,
                   AVG(duration_seconds) AS avg_duration_seconds,
                   MAX(duration_seconds) AS max_duration_seconds,
                   MAX(created_at) AS latest_answered_at
            FROM exam_answers
            WHERE bank_id = ?1
            GROUP BY question_id
            "#,
        )
        .map_err(|err| err.to_string())?;

    let rows = stats_stmt
        .query_map(params![bank_id.clone()], |row| {
            let question_id: String = row.get(0)?;
            Ok((
                question_id,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<f64>>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(|err| err.to_string())?;

    let mut output = Vec::new();
    for row in rows {
        let (question_id, attempt_count, wrong_count, avg_duration_seconds, max_duration_seconds, latest_answered_at) =
            row.map_err(|err| err.to_string())?;
        if !requested.contains(&question_id) {
            continue;
        }
        let latest_is_correct = latest.get(&question_id).and_then(|(value, _)| *value);
        output.push(QuestionPracticeStats {
            bank_id: bank_id.clone(),
            question_id,
            attempt_count,
            wrong_count,
            latest_is_correct,
            latest_answered_at,
            avg_duration_seconds,
            max_duration_seconds,
        });
    }

    Ok(output)
}

#[tauri::command]
pub(crate) fn list_question_flags(bank_id: String) -> Result<Vec<QuestionFlagRow>, String> {
    let conn = open_app_db()?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT bank_id, question_id, flag_type, note, updated_at
            FROM question_flags
            WHERE bank_id = ?1
            ORDER BY updated_at DESC
            "#,
        )
        .map_err(|err| err.to_string())?;

    let flags = stmt.query_map(params![bank_id], |row| {
        Ok(QuestionFlagRow {
            bank_id: row.get(0)?,
            question_id: row.get(1)?,
            flag_type: row.get(2)?,
            note: row.get(3)?,
            updated_at: row.get(4)?,
        })
    })
    .map_err(|err| err.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|err| err.to_string())?;
    Ok(flags)
}

#[tauri::command]
pub(crate) fn set_question_flag(input: SetQuestionFlagInput) -> Result<Vec<QuestionFlagRow>, String> {
    let conn = open_app_db()?;
    let now = Utc::now().to_rfc3339();
    if input.enabled {
        conn.execute(
            r#"
            INSERT INTO question_flags (id, bank_id, question_id, flag_type, note, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
            ON CONFLICT(bank_id, question_id, flag_type)
            DO UPDATE SET note = excluded.note, updated_at = excluded.updated_at
            "#,
            params![Uuid::new_v4().to_string(), input.bank_id, input.question_id, input.flag_type, input.note, now],
        )
        .map_err(|err| err.to_string())?;
    } else {
        conn.execute(
            "DELETE FROM question_flags WHERE bank_id = ?1 AND question_id = ?2 AND flag_type = ?3",
            params![input.bank_id, input.question_id, input.flag_type],
        )
        .map_err(|err| err.to_string())?;
    }
    list_question_flags(input.bank_id)
}

#[tauri::command]
pub(crate) fn list_review_questions(bank_id: String, review_mode: String, session_id: Option<String>) -> Result<Vec<QuestionSummary>, String> {
    let ids = review_question_ids(&bank_id, &review_mode, session_id.as_deref())?;
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let conn = open_bank(&bank_id)?;
    let mut questions = Vec::new();
    for id in &ids {
        match conn.query_row(
            r#"
              SELECT id, sequence_number, topic, question_type, status, page_from, page_to,
                   substr(replace(question_text, char(10), ' '), 1, 180) AS preview,
                   coalesce(recommended_answer, '')
            FROM questions
            WHERE id = ?1
            "#,
            params![id],
            |row| {
                Ok(QuestionSummary {
                    id: row.get(0)?,
                    sequence_number: row.get(1)?,
                    topic: row.get(2)?,
                    question_type: row.get(3)?,
                    status: row.get(4)?,
                    page_from: row.get(5)?,
                    page_to: row.get(6)?,
                    preview: row.get(7)?,
                    recommended_answer: row.get(8)?,
                })
            },
        ) {
            Ok(question) => questions.push(question),
            Err(rusqlite::Error::QueryReturnedNoRows) => continue,
            Err(err) => return Err(err.to_string()),
        }
    }
    let order: HashMap<String, usize> = ids.into_iter().enumerate().map(|(index, id)| (id, index)).collect();
    questions.sort_by_key(|question| order.get(&question.id).copied().unwrap_or(usize::MAX));
    Ok(questions)
}

fn review_question_ids(bank_id: &str, review_mode: &str, session_id: Option<&str>) -> Result<Vec<String>, String> {
    let conn = open_app_db()?;
    if review_mode == "wrong" {
        if let Some(sid) = session_id {
            // Filter wrong questions by specific exam session
            let mut stmt = conn
                .prepare(
                    r#"
                    SELECT question_id
                    FROM exam_answers
                    WHERE bank_id = ?1 AND session_id = ?2 AND is_correct = 0
                    GROUP BY question_id
                    ORDER BY COUNT(*) DESC, MAX(created_at) DESC
                    "#,
                )
                .map_err(|err| err.to_string())?;
            return stmt
                .query_map(params![bank_id, sid], |row| row.get(0))
                .map_err(|err| err.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| err.to_string());
        }
        let mut stmt = conn
            .prepare(
                r#"
                                SELECT question_id
                                FROM (
                                    SELECT question_id,
                                                 COUNT(*) AS wrong_count,
                                                 MAX(created_at) AS last_wrong_at,
                                                 0 AS manual_only
                                    FROM exam_answers
                                    WHERE bank_id = ?1 AND is_correct = 0
                                    GROUP BY question_id

                                    UNION ALL

                                    SELECT question_id,
                                                 0 AS wrong_count,
                                                 updated_at AS last_wrong_at,
                                                 1 AS manual_only
                                    FROM question_flags
                                    WHERE bank_id = ?1 AND flag_type = 'wrong'
                                        AND question_id NOT IN (
                                            SELECT question_id
                                            FROM exam_answers
                                            WHERE bank_id = ?1 AND is_correct = 0
                                        )
                                )
                                ORDER BY wrong_count DESC, last_wrong_at DESC, manual_only ASC
                "#,
            )
            .map_err(|err| err.to_string())?;
        return stmt
            .query_map(params![bank_id], |row| row.get(0))
            .map_err(|err| err.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string());
    }

    let flag_type = match review_mode {
        "favorite" => "favorite",
        "needs_review" => "needs_review",
        "mastered" => "mastered",
        _ => "needs_review",
    };
    let mut stmt = conn
        .prepare("SELECT question_id FROM question_flags WHERE bank_id = ?1 AND flag_type = ?2 ORDER BY updated_at DESC")
        .map_err(|err| err.to_string())?;
    let ids = stmt.query_map(params![bank_id, flag_type], |row| row.get(0))
        .map_err(|err| err.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())?;
    Ok(ids)
}
