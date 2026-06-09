#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! TauriExam 后端入口（胶水层）。
//!
//! 业务实现拆分到以下模块：
//! - [`models`]：跨模块共享的 serde 模型与内部实体。
//! - [`storage`]：路径解析、数据目录迁移、SQLite 打开/建表、题库发现、设置读写。
//! - [`ai`]：AI Responses API、Microsoft Translator REST、AI 翻译后端。
//! - [`banks`]：题库/题目/做题记录/标记/PDF 渲染相关命令。
//! - [`interaction`]：交互题（选择/Hotspot/拖拽）模型构建与判分数据查询。
//! - [`translation`]：AI 问答与单题/批量翻译命令。
//! - [`deck`]：AKP153 设备（DeckHelper）会话接管与心跳。

mod ai;
mod banks;
mod deck;
mod interaction;
mod models;
mod storage;
mod translation;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            banks::list_banks,
            banks::refresh_banks,
            banks::get_app_paths,
            banks::open_data_dir,
            banks::open_question_banks_dir,
            banks::check_bank_health,
            banks::list_questions,
            banks::get_question,
            banks::get_source_pages,
            banks::save_exam_result,
            banks::list_exam_sessions,
            banks::list_exam_answers,
            banks::get_question_practice_stats,
            banks::list_question_flags,
            banks::set_question_flag,
            banks::list_review_questions,
            interaction::get_interaction_model,
            translation::get_ai_settings,
            translation::save_ai_settings,
            translation::test_translator_settings,
            translation::ask_ai_about_question,
            translation::ask_ai_about_question_stream,
            translation::get_cached_translations,
            translation::translate_question,
            translation::batch_translate_bank,
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
