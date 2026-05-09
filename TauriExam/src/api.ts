import { Channel, invoke } from '@tauri-apps/api/core';
import type {
  AiQuestionRequest,
  AiResponseResult,
  AiSettings,
  AiStreamEvent,
  AppPaths,
  BatchTranslateEvent,
  BatchTranslateInput,
  BatchTranslateResult,
  BankHealth,
  BankInfo,
  ExamAnswerDetail,
  ExamSessionSummary,
  PageImage,
  QuestionDetail,
  QuestionFlag,
  QuestionSummary,
  InteractionModel,
  ReviewMode,
  SaveExamInput,
  SavedExam,
  SetQuestionFlagInput,
  TranslateQuestionInput,
  TranslationRow,
  TranslatorTestResult,
} from './types';

export const api = {
  listBanks: () => invoke<BankInfo[]>('list_banks'),
  refreshBanks: () => invoke<BankInfo[]>('refresh_banks'),
  getAppPaths: () => invoke<AppPaths>('get_app_paths'),
  openDataDir: () => invoke<void>('open_data_dir'),
  openQuestionBanksDir: () => invoke<void>('open_question_banks_dir'),
  checkBankHealth: (bankId: string) => invoke<BankHealth>('check_bank_health', { bankId }),
  listQuestionFlags: (bankId: string) => invoke<QuestionFlag[]>('list_question_flags', { bankId }),
  setQuestionFlag: (input: SetQuestionFlagInput) => invoke<QuestionFlag[]>('set_question_flag', { input }),
  listReviewQuestions: (bankId: string, reviewMode: ReviewMode, sessionId?: string) =>
    invoke<QuestionSummary[]>('list_review_questions', { bankId, reviewMode, sessionId: sessionId ?? null }),
  getInteractionModel: (bankId: string, questionId: string) =>
    invoke<InteractionModel>('get_interaction_model', { bankId, questionId }),
  getAiSettings: () => invoke<AiSettings>('get_ai_settings'),
  saveAiSettings: (settings: AiSettings) => invoke<AiSettings>('save_ai_settings', { settings }),
  testTranslatorSettings: (settings: AiSettings) => invoke<TranslatorTestResult>('test_translator_settings', { settings }),
  askAiAboutQuestion: (input: AiQuestionRequest) => invoke<AiResponseResult>('ask_ai_about_question', { input }),
  createAiStreamChannel: (onMessage: (event: AiStreamEvent) => void) => {
    const channel = new Channel<AiStreamEvent>();
    channel.onmessage = onMessage;
    return channel;
  },
  askAiAboutQuestionStream: (input: AiQuestionRequest, onEvent: Channel<AiStreamEvent>) =>
    invoke<AiResponseResult>('ask_ai_about_question_stream', { input, onEvent }),
  getCachedTranslations: (bankId: string, questionId: string, language: string) =>
    invoke<TranslationRow[]>('get_cached_translations', { bankId, questionId, language }),
  translateQuestion: (input: TranslateQuestionInput) => invoke<TranslationRow[]>('translate_question', { input }),
  createBatchTranslateChannel: (onMessage: (event: BatchTranslateEvent) => void) => {
    const channel = new Channel<BatchTranslateEvent>();
    channel.onmessage = onMessage;
    return channel;
  },
  batchTranslateBank: (input: BatchTranslateInput, onEvent: Channel<BatchTranslateEvent>) =>
    invoke<BatchTranslateResult>('batch_translate_bank', { input, onEvent }),
  listQuestions: (bankId: string) => invoke<QuestionSummary[]>('list_questions', { bankId }),
  getQuestion: (bankId: string, questionId: string) =>
    invoke<QuestionDetail>('get_question', { bankId, questionId }),
  getSourcePages: (bankId: string, questionId: string) =>
    invoke<PageImage[]>('get_source_pages', { bankId, questionId }),
  saveExamResult: (input: SaveExamInput) => invoke<SavedExam>('save_exam_result', { input }),
  listExamSessions: () => invoke<ExamSessionSummary[]>('list_exam_sessions'),
  listExamAnswers: (sessionId: string) => invoke<ExamAnswerDetail[]>('list_exam_answers', { sessionId }),
};
