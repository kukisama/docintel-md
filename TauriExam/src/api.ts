import { invoke } from '@tauri-apps/api/core';
import type {
  BankInfo,
  ExamAnswerDetail,
  ExamSessionSummary,
  PageImage,
  QuestionDetail,
  QuestionSummary,
  SaveExamInput,
  SavedExam,
} from './types';

export const api = {
  listBanks: () => invoke<BankInfo[]>('list_banks'),
  listQuestions: (bankId: string) => invoke<QuestionSummary[]>('list_questions', { bankId }),
  getQuestion: (bankId: string, questionId: string) =>
    invoke<QuestionDetail>('get_question', { bankId, questionId }),
  getSourcePages: (bankId: string, questionId: string) =>
    invoke<PageImage[]>('get_source_pages', { bankId, questionId }),
  saveExamResult: (input: SaveExamInput) => invoke<SavedExam>('save_exam_result', { input }),
  listExamSessions: () => invoke<ExamSessionSummary[]>('list_exam_sessions'),
  listExamAnswers: (sessionId: string) => invoke<ExamAnswerDetail[]>('list_exam_answers', { sessionId }),
};
