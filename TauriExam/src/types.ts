export type BankInfo = {
  id: string;
  exam_code: string;
  name: string;
  db_path: string;
  pdf_path: string;
  question_count: number;
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
