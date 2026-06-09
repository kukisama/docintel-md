export type BankInfo = {
  id: string;
  exam_code: string;
  name: string;
  db_path: string;
  pdf_path: string;
  question_count: number;
};

export type AppPaths = {
  data_dir: string;
  app_db_path: string;
  question_banks_dir: string;
  page_cache_dir: string;
};

export type BankHealth = {
  bank_id: string;
  sqlite_ok: boolean;
  pdf_found: boolean;
  question_count: number;
  empty_question_count: number;
  empty_answer_count: number;
  missing_page_count: number;
  max_question_page: number | null;
  pdf_page_count: number | null;
  translation_db_path: string;
  translation_db_exists: boolean;
  translated_count: number;
  warnings: string[];
};

export type QuestionFlag = {
  bank_id: string;
  question_id: string;
  flag_type: string;
  note: string | null;
  updated_at: string;
};

export type SetQuestionFlagInput = {
  bank_id: string;
  question_id: string;
  flag_type: string;
  enabled: boolean;
  note?: string | null;
};

export type ReviewMode = 'wrong' | 'favorite' | 'needs_review' | 'mastered';

export type InteractionOption = {
  key: string;
  text: string;
  group: string | null;
  is_distractor: boolean;
  sort_order: number;
};

export type InteractionRow = {
  id: string;
  prompt: string;
  option_group: string | null;
  correct_selection: string | null;
  sort_order: number;
};

export type InteractionSlot = {
  id: string;
  label: string;
  correct_option: string | null;
  sort_order: number;
};

export type InteractionModel = {
  kind: string;
  can_auto_grade: boolean;
  message: string;
  options: InteractionOption[];
  rows: InteractionRow[];
  slots: InteractionSlot[];
  answer_key: string[];
};

export type DragDropUserAnswer = {
  kind: 'drag_drop';
  slots: Array<{
    slot_id: string;
    option_id: string | null;
  }>;
};

export type HotspotUserAnswer = {
  kind: 'hotspot';
  rows: Array<{
    row_id: string;
    option_id: string | null;
  }>;
};

export type AiSettings = {
  enabled: boolean;
  base_url: string;
  api_version: string;
  api_key: string;
  model: string;
  temperature: number;
  system_prompt: string;
  prompt_analyze: string;
  prompt_summarize: string;
  translation_provider: 'ai' | 'microsoft_translator';
  translator_endpoint: string;
  translator_key: string;
  translator_region: string;
};

export type AiQuestionRequest = {
  bank_id: string;
  question_id: string;
  user_prompt?: string | null;
  action_type?: 'analyze' | 'summarize' | 'freeform' | null;
};

export type AiResponseResult = {
  content: string;
};

export type AiStreamEvent = {
  question_id: string;
  delta: string;
  done: boolean;
  error: string | null;
};

export type TranslateQuestionInput = {
  bank_id: string;
  question_id: string;
  language: string;
  force: boolean;
};

export type BatchTranslateInput = {
  bank_id: string;
  language: string;
  force: boolean;
};

export type BatchTranslateEvent = {
  bank_id: string;
  translation_db_path: string;
  current_index: number;
  total: number;
  translated: number;
  skipped: number;
  failed: number;
  current_question_id: string | null;
  current_sequence_number: number | null;
  message: string;
  done: boolean;
  error: string | null;
};

export type BatchTranslateResult = {
  bank_id: string;
  translation_db_path: string;
  total: number;
  translated: number;
  skipped: number;
  failed: number;
};

export type TranslationRow = {
  field_name: string;
  segment_index: number;
  source_hash: string;
  language: string;
  translated_text: string;
  provider: string;
  model: string;
  version: number;
};

export type TranslatorTestResult = {
  source_text: string;
  translated_text: string;
};

export type QuestionSummary = {
  id: string;
  sequence_number: number;
  topic: string | null;
  question_type: string;
  status: string;
  page_from: number | null;
  page_to: number | null;
  preview: string;
  recommended_answer: string;
};

export type QuestionPracticeStats = {
  bank_id: string;
  question_id: string;
  attempt_count: number;
  wrong_count: number;
  latest_is_correct: boolean | null;
  latest_answered_at: string | null;
  avg_duration_seconds: number | null;
  max_duration_seconds: number | null;
};

export type OptionRow = {
  option_key: string;
  option_text: string;
  sort_order: number;
};

export type AnswerAreaRow = {
  prompt: string;
  source_selection: string | null;
  recommended_selection: string;
  sort_order: number;
};

export type QuestionDetail = {
  id: string;
  sequence_number: number;
  source_question_number: number | null;
  topic: string | null;
  question_type: string;
  status: string;
  source_pages: string | null;
  page_from: number | null;
  page_to: number | null;
  question_text: string;
  options_md: string | null;
  answer_area_md: string | null;
  source_answer: string | null;
  recommended_answer: string | null;
  chinese_judgement: string | null;
  reasoning: string | null;
  notes: string | null;
  question_md: string;
  md_file: string;
  pdf_file: string | null;
  options: OptionRow[];
  answer_areas: AnswerAreaRow[];
};

export type PageImage = {
  page: number;
  path: string;
  data_url: string;
};

export type ExamAnswerInput = {
  question_id: string;
  sequence_number: number;
  user_answer: string;
  correct_answer: string;
  recommended_answer: string;
  is_correct: boolean | null;
  duration_seconds: number;
};

export type SaveExamInput = {
  bank_id: string;
  title: string;
  mode: string;
  duration_seconds: number;
  answers: ExamAnswerInput[];
};

export type SavedExam = {
  id: string;
  total_questions: number;
  correct_count: number;
  wrong_count: number;
};

export type ExamSessionSummary = {
  id: string;
  bank_id: string;
  title: string;
  mode: string;
  started_at: string;
  finished_at: string;
  duration_seconds: number;
  total_questions: number;
  correct_count: number;
  wrong_count: number;
};

export type ExamAnswerDetail = {
  id: string;
  session_id: string;
  bank_id: string;
  question_id: string;
  sequence_number: number;
  user_answer: string;
  correct_answer: string | null;
  recommended_answer: string | null;
  is_correct: boolean | null;
  duration_seconds: number;
  created_at: string;
};

// —— 设备插件（opendecknew / StreamDock Lite）——
// 这些类型对应后端 src-tauri/src/deck 模块；设备不存在时整个功能 no-op。

export type DeckSettings = {
  enabled: boolean;
  base_url: string;
  token: string;
  brightness: number;
};

export type DeckPing = {
  reachable: boolean;
  enabled: boolean;
  device_id: string | null;
};

export type DeckSession = {
  epoch: number;
  event_seq: number;
  lease_ms: number;
};

export type DeckSlotInput = {
  slot_id: string;
  title?: string;
  icon?: string;
  color?: string;
  emit_key?: string;
  clear?: boolean;
};

export type DeckEvent = {
  seq: number;
  slot_id: string;
};

export type DeckEvents = {
  events: DeckEvent[];
  latest_seq: number;
};
