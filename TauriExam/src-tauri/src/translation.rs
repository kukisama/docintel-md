//! AI 设置、AI 题目问答、单题/批量翻译的 Tauri 命令与配套辅助。
//!
//! 底层请求在 `ai` 模块；本模块负责拼 prompt、组织分段、落库与事件回传。

use chrono::Utc;
use rusqlite::{params, Connection};
use tauri::ipc::Channel;
use uuid::Uuid;

use crate::ai::{
    call_ai_translation_api, call_responses_api, call_responses_api_stream, call_translator_batch_api, load_ai_settings,
    question_context, DEFAULT_PROMPT_ANALYZE, DEFAULT_PROMPT_SUMMARIZE,
};
use crate::banks::{get_question, list_questions};
use crate::interaction::get_interaction_model;
use crate::models::{
    AiQuestionRequest, AiResponseResult, AiSettings, AiStreamEvent, BatchTranslateEvent, BatchTranslateInput,
    BatchTranslateResult, QuestionDetail, TranslateQuestionInput, TranslationRow, TranslationSegment, TranslatorTestResult,
};
use crate::storage::{hash_text, open_app_db, open_translation_db, set_setting};

#[tauri::command]
pub(crate) fn get_ai_settings() -> Result<AiSettings, String> {
    load_ai_settings()
}

#[tauri::command]
pub(crate) fn save_ai_settings(settings: AiSettings) -> Result<AiSettings, String> {
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
pub(crate) fn test_translator_settings(settings: AiSettings) -> Result<TranslatorTestResult, String> {
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
pub(crate) async fn ask_ai_about_question(input: AiQuestionRequest) -> Result<AiResponseResult, String> {
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
pub(crate) async fn ask_ai_about_question_stream(input: AiQuestionRequest, on_event: Channel<AiStreamEvent>) -> Result<AiResponseResult, String> {
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
pub(crate) fn get_cached_translations(bank_id: String, question_id: String, language: String) -> Result<Vec<TranslationRow>, String> {
    load_translation_rows(&bank_id, &question_id, &language)
}

#[tauri::command]
pub(crate) async fn translate_question(input: TranslateQuestionInput) -> Result<Vec<TranslationRow>, String> {
    tauri::async_runtime::spawn_blocking(move || translate_question_blocking(input))
        .await
        .map_err(|err| err.to_string())?
}

#[tauri::command]
pub(crate) async fn batch_translate_bank(input: BatchTranslateInput, on_event: Channel<BatchTranslateEvent>) -> Result<BatchTranslateResult, String> {
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
