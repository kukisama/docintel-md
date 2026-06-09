//! 应用内共享的数据结构（serde 模型 + 内部实体）。
//!
//! 仅承载类型定义，不含任何业务逻辑。字段使用 `pub(crate)`，
//! 以便各功能模块（banks / interaction / translation / ai）构造与读取。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub(crate) struct BankInfo {
    pub(crate) id: String,
    pub(crate) exam_code: String,
    pub(crate) name: String,
    pub(crate) db_path: String,
    pub(crate) pdf_path: String,
    pub(crate) question_count: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct AppPaths {
    pub(crate) data_dir: String,
    pub(crate) app_db_path: String,
    pub(crate) question_banks_dir: String,
    pub(crate) page_cache_dir: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct BankHealth {
    pub(crate) bank_id: String,
    pub(crate) sqlite_ok: bool,
    pub(crate) pdf_found: bool,
    pub(crate) question_count: i64,
    pub(crate) empty_question_count: i64,
    pub(crate) empty_answer_count: i64,
    pub(crate) missing_page_count: i64,
    pub(crate) max_question_page: Option<i64>,
    pub(crate) pdf_page_count: Option<i64>,
    pub(crate) translation_db_path: String,
    pub(crate) translation_db_exists: bool,
    pub(crate) translated_count: i64,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct BankEntry {
    pub(crate) id: String,
    pub(crate) exam_code: String,
    pub(crate) name: String,
    pub(crate) db_path: PathBuf,
    pub(crate) pdf_path: Option<PathBuf>,
    pub(crate) cache_dir: PathBuf,
    pub(crate) question_count: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct QuestionSummary {
    pub(crate) id: String,
    pub(crate) sequence_number: i64,
    pub(crate) topic: Option<String>,
    pub(crate) question_type: String,
    pub(crate) status: String,
    pub(crate) page_from: Option<i64>,
    pub(crate) page_to: Option<i64>,
    pub(crate) preview: String,
    pub(crate) recommended_answer: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct QuestionPracticeStats {
    pub(crate) bank_id: String,
    pub(crate) question_id: String,
    pub(crate) attempt_count: i64,
    pub(crate) wrong_count: i64,
    pub(crate) latest_is_correct: Option<bool>,
    pub(crate) latest_answered_at: Option<String>,
    pub(crate) avg_duration_seconds: Option<f64>,
    pub(crate) max_duration_seconds: Option<i64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct OptionRow {
    pub(crate) option_key: String,
    pub(crate) option_text: String,
    pub(crate) sort_order: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct AnswerAreaRow {
    pub(crate) prompt: String,
    pub(crate) source_selection: Option<String>,
    pub(crate) recommended_selection: String,
    pub(crate) sort_order: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct QuestionDetail {
    pub(crate) id: String,
    pub(crate) sequence_number: i64,
    pub(crate) source_question_number: Option<i64>,
    pub(crate) topic: Option<String>,
    pub(crate) question_type: String,
    pub(crate) status: String,
    pub(crate) source_pages: Option<String>,
    pub(crate) page_from: Option<i64>,
    pub(crate) page_to: Option<i64>,
    pub(crate) question_text: String,
    pub(crate) options_md: Option<String>,
    pub(crate) answer_area_md: Option<String>,
    pub(crate) source_answer: Option<String>,
    pub(crate) recommended_answer: Option<String>,
    pub(crate) chinese_judgement: Option<String>,
    pub(crate) reasoning: Option<String>,
    pub(crate) notes: Option<String>,
    pub(crate) question_md: String,
    pub(crate) md_file: String,
    pub(crate) pdf_file: Option<String>,
    pub(crate) options: Vec<OptionRow>,
    pub(crate) answer_areas: Vec<AnswerAreaRow>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PageImage {
    pub(crate) page: i64,
    pub(crate) path: String,
    pub(crate) data_url: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaveExamAnswerInput {
    pub(crate) question_id: String,
    pub(crate) sequence_number: i64,
    pub(crate) user_answer: String,
    pub(crate) correct_answer: String,
    pub(crate) recommended_answer: String,
    pub(crate) is_correct: Option<bool>,
    pub(crate) duration_seconds: i64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SaveExamInput {
    pub(crate) bank_id: String,
    pub(crate) title: String,
    pub(crate) mode: String,
    pub(crate) duration_seconds: i64,
    pub(crate) answers: Vec<SaveExamAnswerInput>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SavedExam {
    pub(crate) id: String,
    pub(crate) total_questions: i64,
    pub(crate) correct_count: i64,
    pub(crate) wrong_count: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct ExamSessionSummary {
    pub(crate) id: String,
    pub(crate) bank_id: String,
    pub(crate) title: String,
    pub(crate) mode: String,
    pub(crate) started_at: String,
    pub(crate) finished_at: String,
    pub(crate) duration_seconds: i64,
    pub(crate) total_questions: i64,
    pub(crate) correct_count: i64,
    pub(crate) wrong_count: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct ExamAnswerDetail {
    pub(crate) id: String,
    pub(crate) session_id: String,
    pub(crate) bank_id: String,
    pub(crate) question_id: String,
    pub(crate) sequence_number: i64,
    pub(crate) user_answer: String,
    pub(crate) correct_answer: Option<String>,
    pub(crate) recommended_answer: Option<String>,
    pub(crate) is_correct: Option<bool>,
    pub(crate) duration_seconds: i64,
    pub(crate) created_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct QuestionFlagRow {
    pub(crate) bank_id: String,
    pub(crate) question_id: String,
    pub(crate) flag_type: String,
    pub(crate) note: Option<String>,
    pub(crate) updated_at: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SetQuestionFlagInput {
    pub(crate) bank_id: String,
    pub(crate) question_id: String,
    pub(crate) flag_type: String,
    pub(crate) enabled: bool,
    pub(crate) note: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct InteractionOption {
    pub(crate) key: String,
    pub(crate) text: String,
    pub(crate) group: Option<String>,
    pub(crate) is_distractor: bool,
    pub(crate) sort_order: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct InteractionRow {
    pub(crate) id: String,
    pub(crate) prompt: String,
    pub(crate) option_group: Option<String>,
    pub(crate) correct_selection: Option<String>,
    pub(crate) sort_order: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct InteractionSlot {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) correct_option: Option<String>,
    pub(crate) sort_order: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct InteractionModel {
    pub(crate) kind: String,
    pub(crate) can_auto_grade: bool,
    pub(crate) message: String,
    pub(crate) options: Vec<InteractionOption>,
    pub(crate) rows: Vec<InteractionRow>,
    pub(crate) slots: Vec<InteractionSlot>,
    pub(crate) answer_key: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct AiSettings {
    pub(crate) enabled: bool,
    pub(crate) base_url: String,
    pub(crate) api_version: String,
    pub(crate) api_key: String,
    pub(crate) model: String,
    pub(crate) temperature: f32,
    pub(crate) system_prompt: String,
    pub(crate) prompt_analyze: String,
    pub(crate) prompt_summarize: String,
    pub(crate) translation_provider: String,
    pub(crate) translator_endpoint: String,
    pub(crate) translator_key: String,
    pub(crate) translator_region: String,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct AiQuestionRequest {
    pub(crate) bank_id: String,
    pub(crate) question_id: String,
    pub(crate) user_prompt: Option<String>,
    pub(crate) action_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AiResponseResult {
    pub(crate) content: String,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct AiStreamEvent {
    pub(crate) question_id: String,
    pub(crate) delta: String,
    pub(crate) done: bool,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TranslateQuestionInput {
    pub(crate) bank_id: String,
    pub(crate) question_id: String,
    pub(crate) language: String,
    pub(crate) force: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BatchTranslateInput {
    pub(crate) bank_id: String,
    pub(crate) language: String,
    pub(crate) force: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct TranslationRow {
    pub(crate) field_name: String,
    pub(crate) segment_index: i64,
    pub(crate) source_hash: String,
    pub(crate) language: String,
    pub(crate) translated_text: String,
    pub(crate) provider: String,
    pub(crate) model: String,
    pub(crate) version: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct TranslatorTestResult {
    pub(crate) source_text: String,
    pub(crate) translated_text: String,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct BatchTranslateEvent {
    pub(crate) bank_id: String,
    pub(crate) translation_db_path: String,
    pub(crate) current_index: i64,
    pub(crate) total: i64,
    pub(crate) translated: i64,
    pub(crate) skipped: i64,
    pub(crate) failed: i64,
    pub(crate) current_question_id: Option<String>,
    pub(crate) current_sequence_number: Option<i64>,
    pub(crate) message: String,
    pub(crate) done: bool,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct BatchTranslateResult {
    pub(crate) bank_id: String,
    pub(crate) translation_db_path: String,
    pub(crate) total: i64,
    pub(crate) translated: i64,
    pub(crate) skipped: i64,
    pub(crate) failed: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct TranslationSegment {
    pub(crate) field_name: String,
    pub(crate) segment_index: i64,
    pub(crate) source_text: String,
}
