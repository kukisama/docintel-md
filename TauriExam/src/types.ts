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

export type AiSettings = {
  enabled: boolean;
  base_url: string;
  api_version: string;
  api_key: string;
  model: string;
  temperature: number;
  translation_provider: 'ai' | 'microsoft_translator';
  translator_endpoint: string;
  translator_key: string;
  translator_region: string;
};

export type AiQuestionRequest = {
  bank_id: string;
  question_id: string;
  user_prompt?: string | null;
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
  question_type: string;
  status: string;
  page_from: number | null;
  page_to: number | null;
  preview: string;
  recommended_answer: string;
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
