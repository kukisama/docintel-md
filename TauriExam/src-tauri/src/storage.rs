//! 文件系统路径、应用数据目录迁移、SQLite 打开与建表、题库发现与设置读写。
//!
//! 这一层是所有"在哪、怎么开"的底座，供 banks / interaction / translation / ai 复用。

use chrono::Utc;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

use crate::models::BankEntry;

const APP_DIR_NAME: &str = "TauriExam";

pub(crate) fn workspace_root() -> Result<PathBuf, String> {
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

pub(crate) fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|path| path.parent().map(Path::to_path_buf))
}

pub(crate) fn app_data_dir() -> Result<PathBuf, String> {
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

pub(crate) fn question_banks_dir() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("question-banks"))
}

pub(crate) fn page_cache_dir() -> Result<PathBuf, String> {
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

pub(crate) fn ensure_app_dirs() -> Result<(), String> {
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

pub(crate) fn table_exists(conn: &Connection, table_name: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![table_name],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
    .unwrap_or(false)
}

pub(crate) fn hash_text(value: &str) -> String {
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

pub(crate) fn discover_banks() -> Result<Vec<BankEntry>, String> {
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

pub(crate) fn find_bank(bank_id: &str) -> Result<BankEntry, String> {
    discover_banks()?
        .into_iter()
        .find(|bank| bank.id == bank_id)
        .ok_or_else(|| format!("Unknown question bank: {bank_id}. Put <name>.sqlite and optional <name>.pdf into question-banks."))
}

fn bank_db_path(bank_id: &str) -> Result<PathBuf, String> {
    Ok(find_bank(bank_id)?.db_path)
}

pub(crate) fn app_db_path() -> Result<PathBuf, String> {
    ensure_app_dirs()?;
    Ok(app_data_dir()?.join("app.sqlite"))
}

pub(crate) fn open_bank(bank_id: &str) -> Result<Connection, String> {
    let path = bank_db_path(bank_id)?;
    Connection::open(&path).map_err(|err| format!("Failed to open {}: {err}", path.display()))
}

pub(crate) fn translation_db_path_for_bank(bank: &BankEntry) -> Result<PathBuf, String> {
    let file_stem = bank
        .db_path
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("无法从题库路径生成翻译库文件名：{}", bank.db_path.display()))?;
    Ok(bank.db_path.with_file_name(format!("{file_stem}.translations.sqlite")))
}

pub(crate) fn open_translation_db(bank_id: &str) -> Result<(Connection, PathBuf), String> {
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
