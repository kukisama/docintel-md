#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use pdfium_render::prelude::*;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::ipc::Channel;
use uuid::Uuid;

mod deck;

const APP_DIR_NAME: &str = "TauriExam";
static PDF_RENDER_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Serialize)]
struct BankInfo {
    id: String,
    exam_code: String,
    name: String,
    db_path: String,
    pdf_path: String,
    question_count: i64,
}

#[derive(Debug, Serialize)]
struct AppPaths {
    data_dir: String,
    app_db_path: String,
    question_banks_dir: String,
    page_cache_dir: String,
}

#[derive(Debug, Serialize)]
struct BankHealth {
    bank_id: String,
    sqlite_ok: bool,
    pdf_found: bool,
    question_count: i64,
    empty_question_count: i64,
    empty_answer_count: i64,
    missing_page_count: i64,
    max_question_page: Option<i64>,
    pdf_page_count: Option<i64>,
    translation_db_path: String,
    translation_db_exists: bool,
    translated_count: i64,
    warnings: Vec<String>,
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
    topic: Option<String>,
    question_type: String,
    status: String,
    page_from: Option<i64>,
    page_to: Option<i64>,
    preview: String,
    recommended_answer: String,
}

#[derive(Debug, Serialize)]
struct QuestionPracticeStats {
    bank_id: String,
    question_id: String,
    attempt_count: i64,
    wrong_count: i64,
    latest_is_correct: Option<bool>,
    latest_answered_at: Option<String>,
    avg_duration_seconds: Option<f64>,
    max_duration_seconds: Option<i64>,
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

#[derive(Debug, Serialize)]
struct QuestionFlagRow {
    bank_id: String,
    question_id: String,
    flag_type: String,
    note: Option<String>,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct SetQuestionFlagInput {
    bank_id: String,
    question_id: String,
    flag_type: String,
    enabled: bool,
    note: Option<String>,
}

#[derive(Debug, Serialize)]
struct InteractionOption {
    key: String,
    text: String,
    group: Option<String>,
    is_distractor: bool,
    sort_order: i64,
}

#[derive(Debug, Serialize)]
struct InteractionRow {
    id: String,
    prompt: String,
    option_group: Option<String>,
    correct_selection: Option<String>,
    sort_order: i64,
}

#[derive(Debug, Serialize)]
struct InteractionSlot {
    id: String,
    label: String,
    correct_option: Option<String>,
    sort_order: i64,
}

#[derive(Debug, Serialize)]
struct InteractionModel {
    kind: String,
    can_auto_grade: bool,
    message: String,
    options: Vec<InteractionOption>,
    rows: Vec<InteractionRow>,
    slots: Vec<InteractionSlot>,
    answer_key: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AiSettings {
    enabled: bool,
    base_url: String,
    api_version: String,
    api_key: String,
    model: String,
    temperature: f32,
    system_prompt: String,
    prompt_analyze: String,
    prompt_summarize: String,
    translation_provider: String,
    translator_endpoint: String,
    translator_key: String,
    translator_region: String,
}

#[derive(Debug, Deserialize, Clone)]
struct AiQuestionRequest {
    bank_id: String,
    question_id: String,
    user_prompt: Option<String>,
    action_type: Option<String>,
}

#[derive(Debug, Serialize)]
struct AiResponseResult {
    content: String,
}

#[derive(Debug, Serialize, Clone)]
struct AiStreamEvent {
    question_id: String,
    delta: String,
    done: bool,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TranslateQuestionInput {
    bank_id: String,
    question_id: String,
    language: String,
    force: bool,
}

#[derive(Debug, Deserialize)]
struct BatchTranslateInput {
    bank_id: String,
    language: String,
    force: bool,
}

#[derive(Debug, Serialize)]
struct TranslationRow {
    field_name: String,
    segment_index: i64,
    source_hash: String,
    language: String,
    translated_text: String,
    provider: String,
    model: String,
    version: i64,
}

#[derive(Debug, Serialize)]
struct TranslatorTestResult {
    source_text: String,
    translated_text: String,
}

#[derive(Debug, Serialize, Clone)]
struct BatchTranslateEvent {
    bank_id: String,
    translation_db_path: String,
    current_index: i64,
    total: i64,
    translated: i64,
    skipped: i64,
    failed: i64,
    current_question_id: Option<String>,
    current_sequence_number: Option<i64>,
    message: String,
    done: bool,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct BatchTranslateResult {
    bank_id: String,
    translation_db_path: String,
    total: i64,
    translated: i64,
    skipped: i64,
    failed: i64,
}

#[derive(Debug, Clone)]
struct TranslationSegment {
    field_name: String,
    segment_index: i64,
    source_text: String,
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

fn app_data_dir() -> Result<PathBuf, String> {
    // Windows: %LOCALAPPDATA%\TauriExam
    #[cfg(target_os = "windows")]
    {
        if let Ok(path) = std::env::var("LOCALAPPDATA") {
            return Ok(PathBuf::from(path).join(APP_DIR_NAME));
        }
    }
    // macOS: ~/Library/Application Support/TauriExam
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return Ok(PathBuf::from(home).join("Library/Application Support").join(APP_DIR_NAME));
        }
    }
    // Linux / fallback: $XDG_DATA_HOME or ~/.local/share/TauriExam
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            return Ok(PathBuf::from(xdg).join(APP_DIR_NAME));
        }
        if let Ok(home) = std::env::var("HOME") {
            return Ok(PathBuf::from(home).join(".local/share").join(APP_DIR_NAME));
        }
    }
    // Dev / unknown environment fallback
    Ok(workspace_root()?.join("output/exam-tool"))
}

fn question_banks_dir() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("question-banks"))
}

fn page_cache_dir() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("cache/pages"))
}

fn legacy_question_bank_roots() -> Result<Vec<PathBuf>, String> {
    let mut roots = Vec::new();
    if let Some(dir) = exe_dir() {
        roots.push(dir.join("question-banks"));
    }
    roots.push(std::env::current_dir().map_err(|err| err.to_string())?.join("question-banks"));
    roots.push(workspace_root()?.join("TauriExam/question-banks"));
    roots.push(workspace_root()?.join("question-banks"));
    roots.dedup();
    Ok(roots)
}

fn migrate_question_bank_files(target: &Path) -> Result<(), String> {
    // Per-file migration: for every legacy root, copy any bank asset that
    // does not yet exist in the target. We intentionally do NOT short-circuit
    // on `has_bank_files(target)` because companion files (e.g. translation
    // databases `<bank>.translations.sqlite`, PDFs) may be added to the
    // legacy folder after the main `.sqlite` has already been migrated.
    for root in legacy_question_bank_roots()? {
        if root == target {
            continue;
        }
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        let mut ensured_target = false;
        for entry in entries {
            let entry = entry.map_err(|err| err.to_string())?;
            let source = entry.path();
            let is_bank_asset = is_supported_sqlite(&source)
                || source.extension().and_then(|ext| ext.to_str()).map(|ext| ext.eq_ignore_ascii_case("pdf")).unwrap_or(false);
            if !is_bank_asset {
                continue;
            }
            let Some(file_name) = source.file_name() else {
                continue;
            };
            let destination = target.join(file_name);
            if destination.exists() {
                continue;
            }
            if !ensured_target {
                fs::create_dir_all(target).map_err(|err| err.to_string())?;
                ensured_target = true;
            }
            fs::copy(&source, &destination).map_err(|err| err.to_string())?;
        }
    }
    Ok(())
}

fn ensure_app_dirs() -> Result<(), String> {
    let data_dir = app_data_dir()?;
    fs::create_dir_all(data_dir.join("logs")).map_err(|err| err.to_string())?;
    fs::create_dir_all(data_dir.join("backups")).map_err(|err| err.to_string())?;
    fs::create_dir_all(page_cache_dir()?).map_err(|err| err.to_string())?;
    let bank_dir = question_banks_dir()?;
    fs::create_dir_all(&bank_dir).map_err(|err| err.to_string())?;
    migrate_question_bank_files(&bank_dir)?;
    migrate_app_db()?;
    Ok(())
}

fn migrate_app_db() -> Result<(), String> {
    let new_path = app_data_dir()?.join("app.sqlite");
    if new_path.exists() {
        return Ok(());
    }
    let old_path = workspace_root()?.join("output/exam-tool/exam-tool.sqlite");
    if old_path.exists() {
        if let Some(parent) = new_path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        fs::copy(old_path, new_path).map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn question_bank_roots() -> Result<Vec<PathBuf>, String> {
    ensure_app_dirs()?;
    Ok(vec![question_banks_dir()?])
}

fn is_supported_sqlite(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ["sqlite", "sqlite3", "db"].iter().any(|candidate| ext.eq_ignore_ascii_case(candidate)))
        .unwrap_or(false)
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

fn table_exists(conn: &Connection, table_name: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![table_name],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .unwrap_or(false)
}

fn hash_text(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}

pub(crate) fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    conn.query_row("SELECT value FROM app_settings WHERE key = ?1", params![key], |row| row.get(0))
        .map(Some)
        .or_else(|err| {
            if matches!(err, rusqlite::Error::QueryReturnedNoRows) {
                Ok(None)
            } else {
                Err(err.to_string())
            }
        })
}

pub(crate) fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        r#"
        INSERT INTO app_settings (key, value, updated_at)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
        "#,
        params![key, value, now],
    )
    .map(|_| ())
    .map_err(|err| err.to_string())
}

const DEFAULT_PROMPT_ANALYZE: &str = "请用中文详细分析这道考试题。要求：1) 解释题干问什么；2) 解释正确答案为什么正确；3) 分析每个错误选项为什么错；4) 提炼知识点；5) 给出记忆方法。";
const DEFAULT_PROMPT_SUMMARIZE: &str = "请用中文简洁总结这道考试题。要求：1) 一句话概括题目在问什么；2) 正确答案是什么；3) 核心考点是什么；4) 关键词列表。不需要逐选项分析。";

fn default_ai_settings() -> AiSettings {
    AiSettings {
        enabled: false,
        base_url: "https://api.openai.com/v1".to_string(),
        api_version: String::new(),
        api_key: String::new(),
        model: "gpt-4.1-mini".to_string(),
        temperature: 0.7,
        system_prompt: String::new(),
        prompt_analyze: DEFAULT_PROMPT_ANALYZE.to_string(),
        prompt_summarize: DEFAULT_PROMPT_SUMMARIZE.to_string(),
        translation_provider: "ai".to_string(),
        translator_endpoint: "https://api.cognitive.microsofttranslator.com".to_string(),
        translator_key: String::new(),
        translator_region: String::new(),
    }
}

fn load_ai_settings() -> Result<AiSettings, String> {
    let conn = open_app_db()?;
    let defaults = default_ai_settings();
    let mut settings = AiSettings {
        enabled: get_setting(&conn, "ai.enabled")?.map(|value| value == "true").unwrap_or(defaults.enabled),
        base_url: get_setting(&conn, "ai.base_url")?.unwrap_or(defaults.base_url),
        api_version: get_setting(&conn, "ai.api_version")?.unwrap_or(defaults.api_version),
        api_key: get_setting(&conn, "ai.api_key")?.unwrap_or(defaults.api_key),
        model: get_setting(&conn, "ai.model")?.unwrap_or(defaults.model),
        temperature: get_setting(&conn, "ai.temperature")?
            .and_then(|value| value.parse::<f32>().ok())
            .unwrap_or(defaults.temperature),
        system_prompt: get_setting(&conn, "ai.system_prompt")?.unwrap_or(defaults.system_prompt),
        prompt_analyze: get_setting(&conn, "ai.prompt_analyze")?.unwrap_or(defaults.prompt_analyze),
        prompt_summarize: get_setting(&conn, "ai.prompt_summarize")?.unwrap_or(defaults.prompt_summarize),
        translation_provider: get_setting(&conn, "translation.provider")?.unwrap_or(defaults.translation_provider),
        translator_endpoint: get_setting(&conn, "translator.endpoint")?.unwrap_or(defaults.translator_endpoint),
        translator_key: get_setting(&conn, "translator.key")?.unwrap_or(defaults.translator_key),
        translator_region: get_setting(&conn, "translator.region")?.unwrap_or(defaults.translator_region),
    };
    if settings.api_version.trim().is_empty() {
        settings.api_version = effective_ai_api_version(&settings);
    }
    Ok(settings)
}

fn question_context(question: &QuestionDetail) -> String {
    let options = question
        .options
        .iter()
        .map(|option| format!("{}. {}", option.option_key, option.option_text))
        .collect::<Vec<_>>()
        .join("\n");
    let answer_areas = question
        .answer_areas
        .iter()
        .map(|row| format!("{} => {}", row.prompt, row.recommended_selection))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "题型: {}\nTopic: {}\n页码: {}\n\n题干:\n{}\n\n选项:\n{}\n\n答案区:\n{}\n\n源答案: {}\n推荐答案: {}\n中文判断: {}\nReasoning:\n{}\nNotes:\n{}",
        question.question_type,
        question.topic.clone().unwrap_or_default(),
        question.source_pages.clone().unwrap_or_default(),
        question.question_text,
        options,
        answer_areas,
        question.source_answer.clone().unwrap_or_default(),
        question.recommended_answer.clone().unwrap_or_default(),
        question.chinese_judgement.clone().unwrap_or_default(),
        question.reasoning.clone().unwrap_or_default(),
        question.notes.clone().unwrap_or_default()
    )
}

fn call_responses_api(settings: &AiSettings, prompt: &str) -> Result<String, String> {
    if !settings.enabled {
        return Err("AI 未启用，请先在控制面板启用 AI。".to_string());
    }
    if settings.api_key.trim().is_empty() {
        return Err("AI API Key 为空，请先在控制面板填写。".to_string());
    }
    let api_version = effective_ai_api_version(settings);
    let url = if api_version.is_empty() {
        format!("{}/responses", settings.base_url.trim_end_matches('/'))
    } else {
        format!(
            "{}/responses?api-version={}",
            settings.base_url.trim_end_matches('/'),
            api_version
        )
    };
    let agent = http_agent();
    let mut body = json!({
        "model": settings.model,
        "temperature": settings.temperature,
        "input": prompt
    });
    if !settings.system_prompt.trim().is_empty() {
        body["instructions"] = json!(settings.system_prompt.trim());
    }
    let mut request = agent.post(&url);
    request = if api_version.is_empty() {
        request.set("Authorization", &format!("Bearer {}", settings.api_key.trim()))
    } else {
        request.set("api-key", settings.api_key.trim())
    };
    let response = request
        .send_json(&body)
        .map_err(|err| err.to_string())?;
    let status = response.status();
    let response_body: Value = response.into_json().map_err(|err| err.to_string())?;
    if status < 200 || status >= 300 {
        return Err(format!("AI 请求失败 ({status}): {response_body}"));
    }
    extract_response_text(&response_body).ok_or_else(|| format!("AI 响应中未找到文本内容: {response_body}"))
}

fn call_responses_api_stream<F>(settings: &AiSettings, prompt: &str, mut on_delta: F) -> Result<String, String>
where
    F: FnMut(&str) -> Result<(), String>,
{
    if !settings.enabled {
        return Err("AI 未启用，请先在控制面板启用 AI。".to_string());
    }
    if settings.api_key.trim().is_empty() {
        return Err("AI API Key 为空，请先在控制面板填写。".to_string());
    }
    let api_version = effective_ai_api_version(settings);
    let url = if api_version.is_empty() {
        format!("{}/responses", settings.base_url.trim_end_matches('/'))
    } else {
        format!(
            "{}/responses?api-version={}",
            settings.base_url.trim_end_matches('/'),
            api_version
        )
    };
    let agent = http_agent();
    let mut body = json!({
        "model": settings.model,
        "temperature": settings.temperature,
        "input": prompt,
        "stream": true
    });
    if !settings.system_prompt.trim().is_empty() {
        body["instructions"] = json!(settings.system_prompt.trim());
    }
    let mut request = agent
        .post(&url)
        .set("Accept", "text/event-stream")
        .set("Cache-Control", "no-cache");
    request = if api_version.is_empty() {
        request.set("Authorization", &format!("Bearer {}", settings.api_key.trim()))
    } else {
        request.set("api-key", settings.api_key.trim())
    };
    let response = request.send_json(&body).map_err(|err| err.to_string())?;
    let status = response.status();
    if status < 200 || status >= 300 {
        let body = response.into_string().unwrap_or_default();
        return Err(format!("AI 请求失败 ({status}): {body}"));
    }
    let mut content = String::new();
    let reader = BufReader::new(response.into_reader());
    for line in reader.lines() {
        let line = line.map_err(|err| err.to_string())?;
        let Some(data) = line.strip_prefix("data:") else {
            if content.is_empty() {
                if let Ok(value) = serde_json::from_str::<Value>(&line) {
                    if let Some(text) = extract_response_text(&value) {
                        content.push_str(&text);
                        on_delta(&text)?;
                    }
                }
            }
            continue;
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let value: Value = match serde_json::from_str(data) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if let Some(delta) = extract_stream_delta(&value) {
            content.push_str(&delta);
            on_delta(&delta)?;
        }
    }
    Ok(content)
}

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(120))
        .build()
}

fn extract_stream_delta(value: &Value) -> Option<String> {
    if value.get("type").and_then(Value::as_str) == Some("response.output_text.delta") {
        return value.get("delta").and_then(Value::as_str).map(str::to_string);
    }
    value.get("delta").and_then(Value::as_str).map(str::to_string)
}

fn effective_ai_api_version(settings: &AiSettings) -> String {
    let configured = settings.api_version.trim();
    if !configured.is_empty() {
        return configured.to_string();
    }
    let base_url = settings.base_url.to_lowercase();
    if (base_url.contains("azure-api.net") || base_url.contains(".openai.azure.com")) && !base_url.contains("/v1") {
        return "2025-03-01-preview".to_string();
    }
    String::new()
}

fn extract_response_text(value: &Value) -> Option<String> {
    if let Some(text) = value.get("output_text").and_then(Value::as_str) {
        if !text.trim().is_empty() {
            return Some(text.to_string());
        }
    }
    let mut parts = Vec::new();
    if let Some(output) = value.get("output").and_then(Value::as_array) {
        for item in output {
            if let Some(content) = item.get("content").and_then(Value::as_array) {
                for part in content {
                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        parts.push(text.to_string());
                    }
                }
            }
        }
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn translator_language(language: &str) -> String {
    match language.trim().to_lowercase().as_str() {
        "zh-cn" | "zh_cn" | "zh" => "zh-Hans".to_string(),
        "zh-tw" | "zh_tw" | "zh-hk" | "zh_hk" => "zh-Hant".to_string(),
        value if value.is_empty() => "zh-Hans".to_string(),
        value => value.to_string(),
    }
}

fn call_translator_batch_api(settings: &AiSettings, segments: &[TranslationSegment], language: &str) -> Result<Vec<String>, String> {
    if settings.translator_key.trim().is_empty() {
        return Err("Microsoft Translator Key 为空，请先在控制面板填写。".to_string());
    }
    let key_diagnostics = translator_key_diagnostics(&settings.translator_key);
    if !key_diagnostics.looks_like_azure_key {
        return Err(format!("Microsoft Translator Key 看起来不是有效 Azure key（{}）。请重新粘贴 Azure 门户 Keys and Endpoint 里的 Key1 或 Key2。", key_diagnostics.summary()));
    }
    if segments.is_empty() {
        return Ok(Vec::new());
    }
    let target = translator_language(language);
    let translator_region = translator_region_for_config(settings)?;
    let url = translator_url_with_target("https://api.cognitive.microsofttranslator.com/translate", &target);
    let agent = http_agent();
    let body = segments
        .iter()
        .map(|segment| json!({ "Text": segment.source_text }))
        .collect::<Vec<_>>();
    let response = agent
        .post(&url)
        .set("Ocp-Apim-Subscription-Key", settings.translator_key.trim())
        .set("Ocp-Apim-Subscription-Region", translator_region.as_str())
        .set("Content-Type", "application/json; charset=UTF-8")
        .send_json(&json!(body))
        .map_err(|err| err.to_string())?;
    let status = response.status();
    let response_text = response.into_string().map_err(|err| err.to_string())?;
    let response_body: Value = serde_json::from_str(&response_text)
        .map_err(|err| format!("Microsoft Translator 响应不是合法 JSON ({status}): {err}; 原文: {response_text}"))?;
    if status < 200 || status >= 300 {
        return Err(format!("Microsoft Translator 请求失败 ({status})，已按官方 Text Translation REST 方式请求 https://api.cognitive.microsofttranslator.com/translate，并传入 Ocp-Apim-Subscription-Key 与 Ocp-Apim-Subscription-Region；key 诊断：{}；响应: {response_body}", key_diagnostics.summary()));
    }
    parse_translator_response(&response_body)
}

struct TranslatorKeyDiagnostics {
    len: usize,
    is_ascii: bool,
    has_password_bullets: bool,
    has_inner_whitespace: bool,
    has_key_label_chars: bool,
    only_token_chars: bool,
    looks_like_azure_key: bool,
}

impl TranslatorKeyDiagnostics {
    fn summary(&self) -> String {
        format!(
            "len={}, ascii={}, contains_password_bullets={}, azure_key_shape={}",
            self.len, self.is_ascii, self.has_password_bullets, self.looks_like_azure_key
        ) + &format!(
            ", inner_whitespace={}, label_chars={}, token_chars_only={}",
            self.has_inner_whitespace, self.has_key_label_chars, self.only_token_chars
        )
    }
}

fn translator_key_diagnostics(key: &str) -> TranslatorKeyDiagnostics {
    let trimmed = key.trim();
    let len = trimmed.chars().count();
    let is_ascii = trimmed.is_ascii();
    let has_password_bullets = trimmed.contains('•') || trimmed.contains('●') || trimmed.contains('*');
    let has_inner_whitespace = trimmed.chars().any(char::is_whitespace);
    let has_key_label_chars = trimmed.contains(':');
    let only_token_chars = trimmed.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '+' || ch == '/' || ch == '=' || ch == '-' || ch == '_');
    let looks_like_azure_key = len >= 20 && len <= 128 && is_ascii && !has_password_bullets && !has_inner_whitespace && !has_key_label_chars && only_token_chars;
    TranslatorKeyDiagnostics { len, is_ascii, has_password_bullets, has_inner_whitespace, has_key_label_chars, only_token_chars, looks_like_azure_key }
}

fn translator_url_with_target(base: &str, target: &str) -> String {
    let separator = if base.contains('?') { '&' } else { '?' };
    let mut url = format!("{base}{separator}");
    if !base.to_lowercase().contains("api-version=") {
        url.push_str("api-version=3.0&");
    }
    url.push_str("to=");
    url.push_str(target);
    url
}

fn translator_region_for_config(settings: &AiSettings) -> Result<String, String> {
    let region = settings.translator_region.trim();
    if region.is_empty() {
        return Err("Microsoft Translator Region 为空；请填写 Azure 门户“位置/区域”里的资源区域，例如 swedencentral。".to_string());
    }
    if !is_supported_translator_endpoint(&settings.translator_endpoint) {
        return Err("Microsoft Translator Endpoint 格式不支持；当前只支持官方 Text Translation endpoint：https://api.cognitive.microsofttranslator.com/。".to_string());
    }
    Ok(region.to_string())
}

fn is_supported_translator_endpoint(endpoint: &str) -> bool {
    let host = normalized_endpoint_host(endpoint);
    host == "api.cognitive.microsofttranslator.com"
}

fn normalized_endpoint_host(endpoint: &str) -> String {
    let endpoint = endpoint.trim().trim_start_matches("https://").trim_start_matches("http://");
    endpoint.split('/').next().unwrap_or_default().to_lowercase()
}

fn parse_translator_response(body: &Value) -> Result<Vec<String>, String> {
    let items = body.as_array().ok_or_else(|| format!("Microsoft Translator 响应不是数组: {body}"))?;
    let mut translations = Vec::new();
    for item in items {
        let text = item
            .get("translations")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|translation| translation.get("text"))
            .and_then(Value::as_str)
            .ok_or_else(|| format!("Microsoft Translator 响应中未找到译文: {body}"))?;
        translations.push(text.to_string());
    }
    Ok(translations)
}

#[cfg(test)]
fn sanitized_translator_url_path(url: &str) -> String {
    let without_scheme = url.split_once("://").map(|(_, right)| right).unwrap_or(url);
    let (host, path) = without_scheme.split_once('/').unwrap_or((without_scheme, ""));
    let host_label = if host.eq_ignore_ascii_case("api.cognitive.microsofttranslator.com") {
        "global-translator"
    } else if host.ends_with(".api.cognitive.microsoft.com") {
        "regional-cognitive"
    } else if host.ends_with(".cognitiveservices.azure.com") {
        "custom-cognitiveservices"
    } else {
        "configured-endpoint"
    };
    format!("{host_label}/{path}")
}

fn call_ai_translation_api(settings: &AiSettings, segments: &[TranslationSegment], language: &str) -> Result<Vec<String>, String> {
    if segments.is_empty() {
        return Ok(Vec::new());
    }
    let items = segments
        .iter()
        .map(|segment| json!({
            "field_name": segment.field_name,
            "segment_index": segment.segment_index,
            "text": segment.source_text
        }))
        .collect::<Vec<_>>();
    let prompt = format!(
        "请把下面考试题页面的结构化 JSON 翻译成 {language}。要求：\n1. 必须只输出 JSON，不要 Markdown，不要解释。\n2. JSON 顶层必须是数组。\n3. 每个元素必须保留 field_name 和 segment_index 原值，并输出 translated_text。\n4. 保留 Microsoft 产品名、考试术语、选项字母、URL、代码、专有名词；不要改答案字母。\n5. 不要遗漏任何元素，输出顺序与输入一致。\n\n输入 JSON：\n{}",
        serde_json::to_string(&items).map_err(|err| err.to_string())?
    );
    let content = call_responses_api(settings, &prompt)?;
    parse_ai_translation_json(&content, segments.len())
}

fn parse_ai_translation_json(content: &str, expected_len: usize) -> Result<Vec<String>, String> {
    let trimmed = content.trim();
    let json_text = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        trimmed
    };
    let value: Value = serde_json::from_str(json_text).map_err(|err| format!("AI 翻译结果不是合法 JSON: {err}; 原文: {content}"))?;
    let items = value.as_array().ok_or_else(|| format!("AI 翻译结果顶层不是数组: {value}"))?;
    if items.len() != expected_len {
        return Err(format!("AI 翻译结果数量不匹配：期望 {expected_len}，实际 {}。", items.len()));
    }
    items
        .iter()
        .map(|item| {
            item.get("translated_text")
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| format!("AI 翻译结果缺少 translated_text: {item}"))
        })
        .collect()
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
            if !is_supported_sqlite(&db_path) {
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
                cache_dir: page_cache_dir()?.join(normalize_bank_id(stem)),
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
    ensure_app_dirs()?;
    Ok(app_data_dir()?.join("app.sqlite"))
}

fn open_bank(bank_id: &str) -> Result<Connection, String> {
    let path = bank_db_path(bank_id)?;
    Connection::open(&path).map_err(|err| format!("Failed to open {}: {err}", path.display()))
}

fn translation_db_path_for_bank(bank: &BankEntry) -> Result<PathBuf, String> {
    let file_stem = bank
        .db_path
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("无法从题库路径生成翻译库文件名：{}", bank.db_path.display()))?;
    Ok(bank.db_path.with_file_name(format!("{file_stem}.translations.sqlite")))
}

fn open_translation_db(bank_id: &str) -> Result<(Connection, PathBuf), String> {
    let bank = find_bank(bank_id)?;
    let path = translation_db_path_for_bank(&bank)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let conn = Connection::open(&path).map_err(|err| format!("Failed to open {}: {err}", path.display()))?;
    init_translation_schema(&conn)?;
    Ok((conn, path))
}

fn init_translation_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS translation_segments (
          id TEXT PRIMARY KEY,
          bank_id TEXT NOT NULL,
          question_id TEXT NOT NULL,
          field_name TEXT NOT NULL,
          segment_index INTEGER NOT NULL,
          source_hash TEXT NOT NULL,
          language TEXT NOT NULL,
          translated_text TEXT NOT NULL,
          provider TEXT NOT NULL,
          model TEXT NOT NULL,
          version INTEGER NOT NULL,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_translation_segments_lookup
        ON translation_segments(bank_id, question_id, language, field_name, segment_index, version);

        CREATE INDEX IF NOT EXISTS idx_translation_segments_resume
        ON translation_segments(bank_id, question_id, language, field_name, segment_index, source_hash);

        CREATE TABLE IF NOT EXISTS translation_meta (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        "#,
    )
    .map_err(|err| err.to_string())
}

pub(crate) fn open_app_db() -> Result<Connection, String> {
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

        CREATE TABLE IF NOT EXISTS app_settings (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

                CREATE TABLE IF NOT EXISTS ai_conversations (
                    id TEXT PRIMARY KEY,
                    bank_id TEXT NOT NULL,
                    question_id TEXT NOT NULL,
                    title TEXT NOT NULL,
                    provider TEXT NOT NULL,
                    model TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS ai_messages (
                    id TEXT PRIMARY KEY,
                    conversation_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY(conversation_id) REFERENCES ai_conversations(id)
                );

                CREATE TABLE IF NOT EXISTS translation_segments (
                    id TEXT PRIMARY KEY,
                    bank_id TEXT NOT NULL,
                    question_id TEXT NOT NULL,
                    field_name TEXT NOT NULL,
                    segment_index INTEGER NOT NULL,
                    source_hash TEXT NOT NULL,
                    language TEXT NOT NULL,
                    translated_text TEXT NOT NULL,
                    provider TEXT NOT NULL,
                    model TEXT NOT NULL,
                    version INTEGER NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
        "#,
    )
    .map_err(|err| err.to_string())
}

#[tauri::command]
fn get_app_paths() -> Result<AppPaths, String> {
    ensure_app_dirs()?;
    Ok(AppPaths {
        data_dir: app_data_dir()?.display().to_string(),
        app_db_path: app_db_path()?.display().to_string(),
        question_banks_dir: question_banks_dir()?.display().to_string(),
        page_cache_dir: page_cache_dir()?.display().to_string(),
    })
}

#[tauri::command]
fn refresh_banks() -> Result<Vec<BankInfo>, String> {
    list_banks()
}

#[tauri::command]
fn open_data_dir() -> Result<(), String> {
    ensure_app_dirs()?;
    open_dir(&app_data_dir()?)
}

#[tauri::command]
fn open_question_banks_dir() -> Result<(), String> {
    ensure_app_dirs()?;
    open_dir(&question_banks_dir()?)
}

#[tauri::command]
fn check_bank_health(bank_id: String) -> Result<BankHealth, String> {
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

#[tauri::command]
fn get_question_practice_stats(bank_id: String, question_ids: Vec<String>) -> Result<Vec<QuestionPracticeStats>, String> {
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
fn list_question_flags(bank_id: String) -> Result<Vec<QuestionFlagRow>, String> {
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
fn set_question_flag(input: SetQuestionFlagInput) -> Result<Vec<QuestionFlagRow>, String> {
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
fn list_review_questions(bank_id: String, review_mode: String, session_id: Option<String>) -> Result<Vec<QuestionSummary>, String> {
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

#[tauri::command]
fn get_interaction_model(bank_id: String, question_id: String) -> Result<InteractionModel, String> {
    let conn = open_bank(&bank_id)?;
    let question = get_question(bank_id, question_id)?;
    let normalized = question.question_type.to_lowercase();
    let interaction_kind = question_interaction_kind(&question.question_md);

    if !question.options.is_empty() {
        return Ok(InteractionModel {
            kind: if normalized.contains("multiple") { "multiple_choice" } else { "single_choice" }.to_string(),
            can_auto_grade: true,
            message: "标准选择题，使用 options 表自动判分。".to_string(),
            options: question
                .options
                .into_iter()
                .map(|option| InteractionOption {
                    key: option.option_key,
                    text: option.option_text,
                    group: None,
                    is_distractor: false,
                    sort_order: option.sort_order,
                })
                .collect(),
            rows: Vec::new(),
            slots: Vec::new(),
            answer_key: Vec::new(),
        });
    }

    if table_exists(&conn, "interaction_targets") && table_exists(&conn, "interaction_options") {
        let options = query_interaction_options(&conn, "interaction_options", &question.id)?;
        let targets = query_unified_interaction_targets(&conn, &question.id)?;
        if !targets.is_empty() && !options.is_empty() {
            if normalized.contains("hotspot") || interaction_kind.as_deref() == Some("dropdown_hotspot") {
                return Ok(InteractionModel {
                    kind: "hotspot".to_string(),
                    can_auto_grade: true,
                    message: "检测到统一 interaction_options / interaction_targets，已按 Hotspot 下拉题渲染。".to_string(),
                    options,
                    rows: targets
                        .into_iter()
                        .map(|target| InteractionRow {
                            id: target.id,
                            prompt: target.label,
                            option_group: target.option_group,
                            correct_selection: target.correct_option,
                            sort_order: target.sort_order,
                        })
                        .collect(),
                    slots: Vec::new(),
                    answer_key: Vec::new(),
                });
            }
            if normalized.contains("drag") || normalized.contains("drop") || matches!(interaction_kind.as_deref(), Some("drag_drop" | "ordered_list")) {
                let is_ordered = interaction_kind.as_deref() == Some("ordered_list");
                let slots: Vec<InteractionSlot> = targets
                    .into_iter()
                    .map(|target| InteractionSlot {
                        id: target.id,
                        label: if is_ordered { format!("{}. {}", target.sort_order, target.label) } else { target.label },
                        correct_option: target.correct_option,
                        sort_order: target.sort_order,
                    })
                    .collect();
                return Ok(InteractionModel {
                    kind: "drag_drop".to_string(),
                    can_auto_grade: true,
                    message: if is_ordered {
                        "检测到统一 interaction_options / interaction_targets，已按有序拖拽题渲染。".to_string()
                    } else {
                        "检测到统一 interaction_options / interaction_targets，已按 Drag Drop 题渲染。".to_string()
                    },
                    options,
                    answer_key: slots.iter().filter_map(|slot| slot.correct_option.clone()).collect(),
                    rows: Vec::new(),
                    slots,
                });
            }
        }
    }

    if normalized.contains("hotspot") && table_exists(&conn, "hotspot_rows") {
        let rows = query_hotspot_rows(&conn, &question.id)?;
        if !rows.is_empty() {
            return Ok(InteractionModel {
                kind: "hotspot".to_string(),
                can_auto_grade: true,
                message: "检测到当前题的 hotspot_rows，已启用结构化 Hotspot 框架。".to_string(),
                options: query_interaction_options(&conn, "hotspot_options", &question.id)?,
                rows,
                slots: Vec::new(),
                answer_key: Vec::new(),
            });
        }
    }

    if normalized.contains("drag") && table_exists(&conn, "drag_slots") {
        let slots = query_drag_slots(&conn, &question.id)?;
        if !slots.is_empty() {
            return Ok(InteractionModel {
                kind: "drag_drop".to_string(),
                can_auto_grade: true,
                message: "检测到当前题的 drag_slots，已启用结构化 Drag Drop 自动判分。".to_string(),
                options: query_interaction_options(&conn, "drag_options", &question.id)?,
                answer_key: slots.iter().filter_map(|slot| slot.correct_option.clone()).collect(),
                rows: Vec::new(),
                slots,
            });
        }
    }

    Ok(InteractionModel {
        kind: "manual".to_string(),
        can_auto_grade: false,
        message: "当前题库缺少结构化 Hotspot/Drag Drop 原始数据，已降级为人工自评；未来 SQL 补齐表后可自动上线。".to_string(),
        options: Vec::new(),
        rows: question
            .answer_areas
            .into_iter()
            .map(|row| InteractionRow {
                id: format!("manual-{}", row.sort_order),
                prompt: row.prompt,
                option_group: None,
                correct_selection: Some(row.recommended_selection),
                sort_order: row.sort_order,
            })
            .collect(),
        slots: Vec::new(),
        answer_key: Vec::new(),
    })
}

fn query_interaction_options(conn: &Connection, table: &str, question_id: &str) -> Result<Vec<InteractionOption>, String> {
    if !table_exists(conn, table) {
        return Ok(Vec::new());
    }
    let sql = if table == "interaction_options" || table == "drag_options" {
        format!("SELECT id, option_text, option_group, coalesce(is_distractor, 0), sort_order FROM {table} WHERE question_id = ?1 ORDER BY sort_order")
    } else {
        format!("SELECT id, option_text, option_group, 0, sort_order FROM {table} WHERE question_id = ?1 ORDER BY sort_order")
    };
    let mut stmt = conn.prepare(&sql).map_err(|err| err.to_string())?;
    let options = stmt.query_map(params![question_id], |row| {
        let is_distractor: i64 = row.get(3)?;
        Ok(InteractionOption {
            key: row.get::<_, String>(0)?,
            text: row.get(1)?,
            group: row.get(2)?,
            is_distractor: is_distractor != 0,
            sort_order: row.get(4)?,
        })
    })
    .map_err(|err| err.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|err| err.to_string())?;
    Ok(options)
}

#[derive(Debug)]
struct UnifiedInteractionTarget {
    id: String,
    sort_order: i64,
    label: String,
    option_group: Option<String>,
    correct_option: Option<String>,
}

fn query_unified_interaction_targets(conn: &Connection, question_id: &str) -> Result<Vec<UnifiedInteractionTarget>, String> {
    let mut stmt = conn
        .prepare("SELECT id, position, target_label, option_group, correct_option FROM interaction_targets WHERE question_id = ?1 ORDER BY position")
        .map_err(|err| err.to_string())?;
    let targets = stmt.query_map(params![question_id], |row| {
        Ok(UnifiedInteractionTarget {
            id: row.get(0)?,
            sort_order: row.get(1)?,
            label: row.get(2)?,
            option_group: row.get(3)?,
            correct_option: row.get(4)?,
        })
    })
    .map_err(|err| err.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|err| err.to_string())?;
    Ok(targets)
}

fn question_interaction_kind(question_md: &str) -> Option<String> {
    question_md.lines().find_map(|line| {
        let trimmed = line.trim();
        let value = trimmed.strip_prefix("- Interaction:")?.trim();
        Some(value.to_lowercase().replace(' ', "_").replace('-', "_"))
    })
}

fn query_hotspot_rows(conn: &Connection, question_id: &str) -> Result<Vec<InteractionRow>, String> {
    let mut stmt = conn
        .prepare("SELECT id, prompt, option_group, correct_selection, sort_order FROM hotspot_rows WHERE question_id = ?1 ORDER BY sort_order")
        .map_err(|err| err.to_string())?;
    let rows = stmt.query_map(params![question_id], |row| {
        Ok(InteractionRow {
            id: row.get(0)?,
            prompt: row.get(1)?,
            option_group: row.get(2)?,
            correct_selection: row.get(3)?,
            sort_order: row.get(4)?,
        })
    })
    .map_err(|err| err.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|err| err.to_string())?;
    Ok(rows)
}

fn query_drag_slots(conn: &Connection, question_id: &str) -> Result<Vec<InteractionSlot>, String> {
    let mut stmt = conn
        .prepare("SELECT id, slot_label, correct_option, sort_order FROM drag_slots WHERE question_id = ?1 ORDER BY sort_order")
        .map_err(|err| err.to_string())?;
    let slots = stmt.query_map(params![question_id], |row| {
        Ok(InteractionSlot {
            id: row.get(0)?,
            label: row.get(1)?,
            correct_option: row.get(2)?,
            sort_order: row.get(3)?,
        })
    })
    .map_err(|err| err.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|err| err.to_string())?;
    Ok(slots)
}

#[tauri::command]
fn get_ai_settings() -> Result<AiSettings, String> {
    load_ai_settings()
}

#[tauri::command]
fn save_ai_settings(settings: AiSettings) -> Result<AiSettings, String> {
    let conn = open_app_db()?;
    set_setting(&conn, "ai.enabled", if settings.enabled { "true" } else { "false" })?;
    set_setting(&conn, "ai.base_url", &settings.base_url)?;
    set_setting(&conn, "ai.api_version", &settings.api_version)?;
    set_setting(&conn, "ai.api_key", &settings.api_key)?;
    set_setting(&conn, "ai.model", &settings.model)?;
    set_setting(&conn, "ai.temperature", &settings.temperature.to_string())?;
    set_setting(&conn, "ai.system_prompt", &settings.system_prompt)?;
    set_setting(&conn, "ai.prompt_analyze", &settings.prompt_analyze)?;
    set_setting(&conn, "ai.prompt_summarize", &settings.prompt_summarize)?;
    set_setting(&conn, "translation.provider", &settings.translation_provider)?;
    set_setting(&conn, "translator.endpoint", &settings.translator_endpoint)?;
    set_setting(&conn, "translator.key", &settings.translator_key)?;
    set_setting(&conn, "translator.region", &settings.translator_region)?;
    load_ai_settings()
}

#[tauri::command]
fn test_translator_settings(settings: AiSettings) -> Result<TranslatorTestResult, String> {
    let source_text = "Hello".to_string();
    let translated = call_translator_batch_api(
        &settings,
        &[TranslationSegment {
            field_name: "probe".to_string(),
            segment_index: 0,
            source_text: source_text.clone(),
        }],
        "zh-CN",
    )?;
    let translated_text = translated.into_iter().next().ok_or_else(|| "Microsoft Translator 测试没有返回译文。".to_string())?;
    Ok(TranslatorTestResult { source_text, translated_text })
}

#[tauri::command]
async fn ask_ai_about_question(input: AiQuestionRequest) -> Result<AiResponseResult, String> {
    tauri::async_runtime::spawn_blocking(move || ask_ai_about_question_blocking(input))
        .await
        .map_err(|err| err.to_string())?
}

fn ask_ai_about_question_blocking(input: AiQuestionRequest) -> Result<AiResponseResult, String> {
    let settings = load_ai_settings()?;
    let question = get_question(input.bank_id.clone(), input.question_id.clone())?;
    let prompt = ai_question_prompt(&input, &question, &settings);
    let content = call_responses_api(&settings, &prompt)?;
    save_ai_exchange(&input.bank_id, &input.question_id, &settings, &prompt, &content)?;
    Ok(AiResponseResult { content })
}

#[tauri::command]
async fn ask_ai_about_question_stream(input: AiQuestionRequest, on_event: Channel<AiStreamEvent>) -> Result<AiResponseResult, String> {
    tauri::async_runtime::spawn_blocking(move || ask_ai_about_question_stream_blocking(input, on_event))
        .await
        .map_err(|err| err.to_string())?
}

fn ask_ai_about_question_stream_blocking(input: AiQuestionRequest, on_event: Channel<AiStreamEvent>) -> Result<AiResponseResult, String> {
    let question_id = input.question_id.clone();
    let settings = load_ai_settings()?;
    let question = get_question(input.bank_id.clone(), input.question_id.clone())?;
    let prompt = ai_question_prompt(&input, &question, &settings);
    let result = call_responses_api_stream(&settings, &prompt, |delta| {
        on_event
            .send(AiStreamEvent {
                question_id: question_id.clone(),
                delta: delta.to_string(),
                done: false,
                error: None,
            })
            .map_err(|err| err.to_string())
    });
    match result {
        Ok(mut content) => {
            if content.trim().is_empty() {
                content = call_responses_api(&settings, &prompt)?;
                on_event
                    .send(AiStreamEvent {
                        question_id: question_id.clone(),
                        delta: content.clone(),
                        done: false,
                        error: None,
                    })
                    .map_err(|err| err.to_string())?;
            }
            save_ai_exchange(&input.bank_id, &input.question_id, &settings, &prompt, &content)?;
            on_event
                .send(AiStreamEvent {
                    question_id,
                    delta: String::new(),
                    done: true,
                    error: None,
                })
                .map_err(|err| err.to_string())?;
            Ok(AiResponseResult { content })
        }
        Err(err) => {
            let _ = on_event.send(AiStreamEvent {
                question_id,
                delta: String::new(),
                done: true,
                error: Some(err.clone()),
            });
            Err(err)
        }
    }
}

fn ai_question_prompt(input: &AiQuestionRequest, question: &QuestionDetail, settings: &AiSettings) -> String {
    let action = input.action_type.as_deref().unwrap_or("analyze");
    match action {
        "summarize" => {
            let template = if settings.prompt_summarize.trim().is_empty() {
                DEFAULT_PROMPT_SUMMARIZE
            } else {
                settings.prompt_summarize.trim()
            };
            format!("{template}\n\n题目上下文：\n{}", question_context(question))
        }
        "freeform" => {
            let user_text = input.user_prompt.as_deref().unwrap_or("").trim();
            if user_text.is_empty() {
                format!("请帮我看看这道题。\n\n题目上下文：\n{}", question_context(question))
            } else {
                format!("{user_text}\n\n题目上下文：\n{}", question_context(question))
            }
        }
        _ => {
            // "analyze" — default
            let template = if settings.prompt_analyze.trim().is_empty() {
                DEFAULT_PROMPT_ANALYZE
            } else {
                settings.prompt_analyze.trim()
            };
            let user_extra = input.user_prompt.as_deref().unwrap_or("").trim();
            if user_extra.is_empty() {
                format!("{template}\n\n题目上下文：\n{}", question_context(question))
            } else {
                format!("{template}\n\n用户追问或补充：{user_extra}\n\n题目上下文：\n{}", question_context(question))
            }
        }
    }
}

fn save_ai_exchange(bank_id: &str, question_id: &str, settings: &AiSettings, prompt: &str, content: &str) -> Result<(), String> {
    let conn = open_app_db()?;
    let now = Utc::now().to_rfc3339();
    let conversation_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO ai_conversations (id, bank_id, question_id, title, provider, model, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'openai-compatible', ?5, ?6, ?6)",
        params![conversation_id, bank_id, question_id, "题目分析", settings.model, now],
    )
    .map_err(|err| err.to_string())?;
    for (role, text) in [("user", prompt), ("assistant", content)] {
        conn.execute(
            "INSERT INTO ai_messages (id, conversation_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![Uuid::new_v4().to_string(), conversation_id, role, text, now],
        )
        .map_err(|err| err.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn get_cached_translations(bank_id: String, question_id: String, language: String) -> Result<Vec<TranslationRow>, String> {
    load_translation_rows(&bank_id, &question_id, &language)
}

#[tauri::command]
async fn translate_question(input: TranslateQuestionInput) -> Result<Vec<TranslationRow>, String> {
    tauri::async_runtime::spawn_blocking(move || translate_question_blocking(input))
        .await
        .map_err(|err| err.to_string())?
}

#[tauri::command]
async fn batch_translate_bank(input: BatchTranslateInput, on_event: Channel<BatchTranslateEvent>) -> Result<BatchTranslateResult, String> {
    tauri::async_runtime::spawn_blocking(move || batch_translate_bank_blocking(input, on_event))
        .await
        .map_err(|err| err.to_string())?
}

fn batch_translate_bank_blocking(input: BatchTranslateInput, on_event: Channel<BatchTranslateEvent>) -> Result<BatchTranslateResult, String> {
    let settings = load_ai_settings()?;
    let questions = list_questions(input.bank_id.clone())?;
    let total = questions.len() as i64;
    let (conn, translation_db_path) = open_translation_db(&input.bank_id)?;
    let translation_db_path_text = translation_db_path.display().to_string();
    let mut translated = 0_i64;
    let mut skipped = 0_i64;
    let mut failed = 0_i64;

    send_batch_translate_event(
        &on_event,
        BatchTranslateEvent {
            bank_id: input.bank_id.clone(),
            translation_db_path: translation_db_path_text.clone(),
            current_index: 0,
            total,
            translated,
            skipped,
            failed,
            current_question_id: None,
            current_sequence_number: None,
            message: format!("准备批量翻译，共 {total} 题。"),
            done: false,
            error: None,
        },
    );

    for (index, summary) in questions.iter().enumerate() {
        let current_index = index as i64 + 1;
        let question = get_question(input.bank_id.clone(), summary.id.clone())?;
        let segments = translation_segments_for_question(&input.bank_id, &question)?;

        if !input.force && translation_package_has_current_segments(&conn, &input.bank_id, &summary.id, &input.language, &segments)? {
            skipped += 1;
            send_batch_translate_event(
                &on_event,
                BatchTranslateEvent {
                    bank_id: input.bank_id.clone(),
                    translation_db_path: translation_db_path_text.clone(),
                    current_index,
                    total,
                    translated,
                    skipped,
                    failed,
                    current_question_id: Some(summary.id.clone()),
                    current_sequence_number: Some(summary.sequence_number),
                    message: format!("第 {current_index}/{total} 题已存在翻译，跳过。"),
                    done: false,
                    error: None,
                },
            );
            continue;
        }

        send_batch_translate_event(
            &on_event,
            BatchTranslateEvent {
                bank_id: input.bank_id.clone(),
                translation_db_path: translation_db_path_text.clone(),
                current_index,
                total,
                translated,
                skipped,
                failed,
                current_question_id: Some(summary.id.clone()),
                current_sequence_number: Some(summary.sequence_number),
                message: format!("正在翻译第 {current_index}/{total} 题（Q{}）...", summary.sequence_number),
                done: false,
                error: None,
            },
        );

        if input.force {
            clear_translation_rows_in_conn(&conn, &input.bank_id, &summary.id, &input.language)?;
        }

        let translation_result: Result<(Vec<String>, String, String), String> = if settings.translation_provider == "microsoft_translator" {
            call_translator_batch_api(&settings, &segments, &input.language)
                .map(|rows| (rows, "microsoft-translator".to_string(), "text-translation-v3".to_string()))
        } else {
            call_ai_translation_api(&settings, &segments, &input.language).map(|rows| (rows, "ai".to_string(), settings.model.clone()))
        };
        let (translated_segments, provider, model) = match translation_result {
            Ok(value) => value,
            Err(err) => {
                failed += 1;
                let message = format!("第 {current_index}/{total} 题翻译失败（Q{}）：{err}", summary.sequence_number);
                send_batch_translate_event(
                    &on_event,
                    BatchTranslateEvent {
                        bank_id: input.bank_id.clone(),
                        translation_db_path: translation_db_path_text.clone(),
                        current_index,
                        total,
                        translated,
                        skipped,
                        failed,
                        current_question_id: Some(summary.id.clone()),
                        current_sequence_number: Some(summary.sequence_number),
                        message: message.clone(),
                        done: false,
                        error: Some(message.clone()),
                    },
                );
                return Err(message);
            }
        };
        if translated_segments.len() != segments.len() {
            failed += 1;
            let message = format!("第 {current_index}/{total} 题翻译结果数量不匹配：期望 {}，实际 {}。", segments.len(), translated_segments.len());
            send_batch_translate_event(
                &on_event,
                BatchTranslateEvent {
                    bank_id: input.bank_id.clone(),
                    translation_db_path: translation_db_path_text.clone(),
                    current_index,
                    total,
                    translated,
                    skipped,
                    failed,
                    current_question_id: Some(summary.id.clone()),
                    current_sequence_number: Some(summary.sequence_number),
                    message: message.clone(),
                    done: false,
                    error: Some(message.clone()),
                },
            );
            return Err(message);
        }

        for (segment, translated_text) in segments.iter().zip(translated_segments.iter()) {
            save_translation_segment_in_conn(
                &conn,
                &input.bank_id,
                &summary.id,
                &segment.field_name,
                segment.segment_index,
                &segment.source_text,
                &input.language,
                translated_text,
                &provider,
                &model,
            )?;
        }
        translated += 1;
        send_batch_translate_event(
            &on_event,
            BatchTranslateEvent {
                bank_id: input.bank_id.clone(),
                translation_db_path: translation_db_path_text.clone(),
                current_index,
                total,
                translated,
                skipped,
                failed,
                current_question_id: Some(summary.id.clone()),
                current_sequence_number: Some(summary.sequence_number),
                message: format!("第 {current_index}/{total} 题翻译完成。"),
                done: false,
                error: None,
            },
        );
    }

    let result = BatchTranslateResult {
        bank_id: input.bank_id.clone(),
        translation_db_path: translation_db_path_text.clone(),
        total,
        translated,
        skipped,
        failed,
    };
    send_batch_translate_event(
        &on_event,
        BatchTranslateEvent {
            bank_id: input.bank_id,
            translation_db_path: translation_db_path_text,
            current_index: total,
            total,
            translated,
            skipped,
            failed,
            current_question_id: None,
            current_sequence_number: None,
            message: format!("批量翻译完成：新翻译 {translated} 题，跳过 {skipped} 题，失败 {failed} 题。"),
            done: true,
            error: None,
        },
    );
    Ok(result)
}

fn send_batch_translate_event(on_event: &Channel<BatchTranslateEvent>, event: BatchTranslateEvent) {
    let _ = on_event.send(event);
}

fn translation_package_has_current_segments(
    conn: &Connection,
    bank_id: &str,
    question_id: &str,
    language: &str,
    segments: &[TranslationSegment],
) -> Result<bool, String> {
    if segments.is_empty() {
        return Ok(true);
    }
    for segment in segments {
        let source_hash = hash_text(&segment.source_text);
        let count = conn
            .query_row(
                r#"
                SELECT COUNT(*)
                FROM translation_segments
                WHERE bank_id = ?1 AND question_id = ?2 AND field_name = ?3
                  AND segment_index = ?4 AND language = ?5 AND source_hash = ?6
                "#,
                params![bank_id, question_id, segment.field_name, segment.segment_index, language, source_hash],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|err| err.to_string())?;
        if count == 0 {
            return Ok(false);
        }
    }
    Ok(true)
}

fn translate_question_blocking(input: TranslateQuestionInput) -> Result<Vec<TranslationRow>, String> {
    let question = get_question(input.bank_id.clone(), input.question_id.clone())?;
    let segments = translation_segments_for_question(&input.bank_id, &question)?;
    if !input.force {
        let (conn, _) = open_translation_db(&input.bank_id)?;
        if translation_package_has_current_segments(&conn, &input.bank_id, &input.question_id, &input.language, &segments)? {
            return load_translation_rows_from_conn(&conn, &input.bank_id, &input.question_id, &input.language);
        }
    }
    let settings = load_ai_settings()?;
    let (translated_segments, provider, model) = if settings.translation_provider == "microsoft_translator" {
        (
            call_translator_batch_api(&settings, &segments, &input.language)?,
            "microsoft-translator".to_string(),
            "text-translation-v3".to_string(),
        )
    } else {
        (call_ai_translation_api(&settings, &segments, &input.language)?, "ai".to_string(), settings.model.clone())
    };
    if translated_segments.len() != segments.len() {
        return Err(format!("翻译结果数量不匹配：期望 {}，实际 {}。", segments.len(), translated_segments.len()));
    }
    if input.force {
        clear_translation_rows(&input.bank_id, &input.question_id, &input.language)?;
    }
    let mut rows = Vec::new();
    for (segment, translated) in segments.iter().zip(translated_segments.iter()) {
        rows.push(save_translation_segment(
            &input.bank_id,
            &input.question_id,
            &segment.field_name,
            segment.segment_index,
            &segment.source_text,
            &input.language,
            translated,
            &provider,
            &model,
        )?);
    }
    Ok(rows)
}

fn clear_translation_rows(bank_id: &str, question_id: &str, language: &str) -> Result<(), String> {
    let (conn, _) = open_translation_db(bank_id)?;
    clear_translation_rows_in_conn(&conn, bank_id, question_id, language)
}

fn translation_segments_from_question(question: &QuestionDetail) -> Vec<TranslationSegment> {
    let mut segments = Vec::new();
    push_translation_segment(&mut segments, "question_text", 0, &question.question_text);
    for option in &question.options {
        push_translation_segment(&mut segments, &format!("option:{}", option.option_key), 0, &option.option_text);
    }
    for answer_area in &question.answer_areas {
        push_translation_segment(&mut segments, &format!("answer_area_prompt:{}", answer_area.sort_order), 0, &answer_area.prompt);
        if let Some(source_selection) = &answer_area.source_selection {
            push_translation_segment(&mut segments, &format!("answer_area_source:{}", answer_area.sort_order), 0, source_selection);
        }
        push_translation_segment(
            &mut segments,
            &format!("answer_area_recommended:{}", answer_area.sort_order),
            0,
            &answer_area.recommended_selection,
        );
    }
    if let Some(source_answer) = &question.source_answer {
        push_translation_segment(&mut segments, "source_answer", 0, source_answer);
    }
    if let Some(recommended_answer) = &question.recommended_answer {
        push_translation_segment(&mut segments, "recommended_answer", 0, recommended_answer);
    }
    if let Some(chinese_judgement) = &question.chinese_judgement {
        push_translation_segment(&mut segments, "chinese_judgement", 0, chinese_judgement);
    }
    if let Some(reasoning) = &question.reasoning {
        push_translation_segment(&mut segments, "reasoning", 0, reasoning);
    }
    if let Some(notes) = &question.notes {
        push_translation_segment(&mut segments, "notes", 0, notes);
    }
    segments
}

fn translation_segments_for_question(bank_id: &str, question: &QuestionDetail) -> Result<Vec<TranslationSegment>, String> {
    let mut segments = translation_segments_from_question(question);
    let model = get_interaction_model(bank_id.to_string(), question.id.clone())?;
    if model.kind == "drag_drop" || model.kind == "hotspot" {
        for option in &model.options {
            push_translation_segment(&mut segments, &format!("interaction_option:{}", option.key), 0, &option.text);
        }
        for slot in &model.slots {
            push_translation_segment(&mut segments, &format!("interaction_target:{}", slot.id), 0, &slot.label);
        }
        for row in &model.rows {
            push_translation_segment(&mut segments, &format!("interaction_target:{}", row.id), 0, &row.prompt);
        }
    }
    Ok(segments)
}

fn push_translation_segment(segments: &mut Vec<TranslationSegment>, field_name: &str, segment_index: i64, source_text: &str) {
    if source_text.trim().is_empty() {
        return;
    }
    segments.push(TranslationSegment {
        field_name: field_name.to_string(),
        segment_index,
        source_text: source_text.trim().to_string(),
    });
}

fn save_translation_segment(
    bank_id: &str,
    question_id: &str,
    field_name: &str,
    segment_index: i64,
    source_text: &str,
    language: &str,
    translated_text: &str,
    provider: &str,
    model: &str,
) -> Result<TranslationRow, String> {
    let (conn, _) = open_translation_db(bank_id)?;
    save_translation_segment_in_conn(
        &conn,
        bank_id,
        question_id,
        field_name,
        segment_index,
        source_text,
        language,
        translated_text,
        provider,
        model,
    )
}

fn save_translation_segment_in_conn(
    conn: &Connection,
    bank_id: &str,
    question_id: &str,
    field_name: &str,
    segment_index: i64,
    source_text: &str,
    language: &str,
    translated_text: &str,
    provider: &str,
    model: &str,
) -> Result<TranslationRow, String> {
    let now = Utc::now().to_rfc3339();
    let source_hash = hash_text(source_text);
    let version = conn
        .query_row(
            "SELECT coalesce(max(version), 0) + 1 FROM translation_segments WHERE bank_id = ?1 AND question_id = ?2 AND field_name = ?3 AND segment_index = ?4 AND language = ?5",
            params![bank_id, question_id, field_name, segment_index, language],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(1);
    conn.execute(
        r#"
        INSERT INTO translation_segments
        (id, bank_id, question_id, field_name, segment_index, source_hash, language, translated_text, provider, model, version, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)
        "#,
        params![Uuid::new_v4().to_string(), bank_id, question_id, field_name, segment_index, source_hash, language, translated_text, provider, model, version, now],
    )
    .map_err(|err| err.to_string())?;
    Ok(TranslationRow {
        field_name: field_name.to_string(),
        segment_index,
        source_hash,
        language: language.to_string(),
        translated_text: translated_text.to_string(),
        provider: provider.to_string(),
        model: model.to_string(),
        version,
    })
}

fn clear_translation_rows_in_conn(conn: &Connection, bank_id: &str, question_id: &str, language: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM translation_segments WHERE bank_id = ?1 AND question_id = ?2 AND language = ?3",
        params![bank_id, question_id, language],
    )
    .map(|_| ())
    .map_err(|err| err.to_string())
}

fn load_translation_rows(bank_id: &str, question_id: &str, language: &str) -> Result<Vec<TranslationRow>, String> {
    let (conn, _) = open_translation_db(bank_id)?;
    load_translation_rows_from_conn(&conn, bank_id, question_id, language)
}

fn load_translation_rows_from_conn(conn: &Connection, bank_id: &str, question_id: &str, language: &str) -> Result<Vec<TranslationRow>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT field_name, segment_index, source_hash, language, translated_text, provider, model, version
            FROM translation_segments t
            WHERE bank_id = ?1 AND question_id = ?2 AND language = ?3
              AND version = (
                SELECT max(version) FROM translation_segments
                WHERE bank_id = t.bank_id AND question_id = t.question_id AND field_name = t.field_name
                  AND segment_index = t.segment_index AND language = t.language
              )
            ORDER BY field_name, segment_index
            "#,
        )
        .map_err(|err| err.to_string())?;
    let rows = stmt.query_map(params![bank_id, question_id, language], |row| {
        Ok(TranslationRow {
            field_name: row.get(0)?,
            segment_index: row.get(1)?,
            source_hash: row.get(2)?,
            language: row.get(3)?,
            translated_text: row.get(4)?,
            provider: row.get(5)?,
            model: row.get(6)?,
            version: row.get(7)?,
        })
    })
    .map_err(|err| err.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|err| err.to_string())?;
    Ok(rows)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            list_banks,
            refresh_banks,
            get_app_paths,
            open_data_dir,
            open_question_banks_dir,
            check_bank_health,
            list_questions,
            get_question,
            get_source_pages,
            save_exam_result,
            list_exam_sessions,
            list_exam_answers,
            get_question_practice_stats,
            list_question_flags,
            set_question_flag,
            list_review_questions,
            get_interaction_model,
            get_ai_settings,
            save_ai_settings,
            test_translator_settings,
            ask_ai_about_question,
            ask_ai_about_question_stream,
            get_cached_translations,
            translate_question,
            batch_translate_bank,
            deck::deck_get_settings,
            deck::deck_save_settings,
            deck::deck_ping,
            deck::deck_takeover,
            deck::deck_heartbeat,
            deck::deck_push_slots,
            deck::deck_set_brightness,
            deck::deck_host_brightness,
            deck::deck_poll_events,
            deck::deck_release
        ])
        .run(tauri::generate_context!())
        .expect("error while running TauriExam");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "uses saved live Microsoft Translator credentials"]
    fn translator_live_hello_with_saved_settings() {
        let settings = load_ai_settings().expect("load saved app settings");
        assert!(
            !settings.translator_key.trim().is_empty(),
            "translator key is empty in saved settings"
        );
        assert!(
            !settings.translator_endpoint.trim().is_empty(),
            "translator endpoint is empty in saved settings"
        );
        let region = translator_region_for_config(&settings).expect("resolve translator region");
        let url = translator_url_with_target("https://api.cognitive.microsofttranslator.com/translate", "zh-Hans");
        println!("translator endpoint shape: {}", sanitized_translator_endpoint_shape(&settings.translator_endpoint));
        println!("translator region: {region}");
        println!("translator key edge: {}", test_key_edge(&settings.translator_key));
        println!("translator request path: {}", super::sanitized_translator_url_path(&url));
        let result = call_translator_batch_api(
            &settings,
            &[TranslationSegment {
                field_name: "probe".to_string(),
                segment_index: 0,
                source_text: "Hello".to_string(),
            }],
            "zh-CN",
        )
        .expect("translate Hello with saved Microsoft Translator settings");
        assert_eq!(result.len(), 1);
        assert!(
            result[0].contains('你') || result[0].contains('好') || result[0].to_lowercase() != "hello",
            "unexpected translation result: {}",
            result[0]
        );
        println!("translator live result: {}", result[0]);
    }

    #[test]
    #[ignore = "uses saved live Microsoft Translator credentials"]
    fn translator_live_official_text_rest_with_saved_settings() {
        let mut settings = load_ai_settings().expect("load saved app settings");
        if let Ok(endpoint) = std::env::var("TRANSLATOR_TEST_ENDPOINT") {
            settings.translator_endpoint = endpoint;
        }
        if let Ok(region) = std::env::var("TRANSLATOR_TEST_REGION") {
            settings.translator_region = region;
        }
        assert!(
            !settings.translator_key.trim().is_empty(),
            "translator key is empty in saved settings"
        );
        let region = translator_region_for_config(&settings).expect("resolve translator region");
        let url = translator_url_with_target("https://api.cognitive.microsofttranslator.com/translate", "zh-Hans");
        let agent = super::http_agent();
        let body = json!([{ "Text": "Hello" }]);

        println!("official text endpoint shape: {}", sanitized_translator_endpoint_shape(&settings.translator_endpoint));
        println!("official text region: {region}");
        println!("official text key edge: {}", test_key_edge(&settings.translator_key));
        println!("official text url: {}", super::sanitized_translator_url_path(&url));

        let direct = agent
            .post(&url)
            .set("Ocp-Apim-Subscription-Key", settings.translator_key.trim())
            .set("Ocp-Apim-Subscription-Region", region.as_str())
            .set("Content-Type", "application/json; charset=UTF-8")
            .send_json(&body)
            .expect("send official key+region request");
        let direct_status = direct.status();
        let direct_text = direct.into_string().expect("read official key+region response");
        println!("official key+region status: {direct_status}");
        println!("official key+region body: {direct_text}");

        assert!(
            direct_status >= 200 && direct_status < 300,
            "official Translator Text REST key+region request failed; status={direct_status}"
        );
    }

    fn sanitized_translator_endpoint_shape(endpoint: &str) -> &'static str {
        let lower = endpoint.to_lowercase();
        if lower.contains(".cognitiveservices.azure.com") {
            "custom-cognitiveservices"
        } else if lower.contains(".cognitive.microsofttranslator.com") {
            "translator-global-or-regional"
        } else {
            "other"
        }
    }

    fn test_key_edge(key: &str) -> String {
        let trimmed = key.trim();
        let prefix = trimmed.chars().take(2).collect::<String>();
        let suffix = trimmed.chars().rev().take(2).collect::<Vec<_>>().into_iter().rev().collect::<String>();
        format!("{prefix}...{suffix} (len={})", trimmed.chars().count())
    }

}
