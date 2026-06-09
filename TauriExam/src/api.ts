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
  DeckEvents,
  DeckPing,
  DeckSession,
  DeckSettings,
  DeckSlotInput,
  ExamAnswerDetail,
  ExamSessionSummary,
  PageImage,
  QuestionDetail,
  QuestionFlag,
  QuestionPracticeStats,
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
  getQuestionPracticeStats: (bankId: string, questionIds: string[]) =>
    invoke<QuestionPracticeStats[]>('get_question_practice_stats', { bankId, questionIds }),

  // —— 设备插件（opendecknew）。后端 deck 模块；设备不存在时这些调用静默失败由调用方吞掉 ——
  deckGetSettings: () => invoke<DeckSettings>('deck_get_settings'),
  deckSaveSettings: (settings: DeckSettings) => invoke<DeckSettings>('deck_save_settings', { settings }),
  deckPing: () => invoke<DeckPing>('deck_ping'),
  deckTakeover: () => invoke<DeckSession>('deck_takeover'),
  deckHeartbeat: (epoch: number) => invoke<void>('deck_heartbeat', { epoch }),
  deckPushSlots: (epoch: number, clearFirst: boolean, slots: DeckSlotInput[]) =>
    invoke<void>('deck_push_slots', { epoch, clearFirst, slots }),
  deckSetBrightness: (epoch: number, brightness: number) =>
    invoke<void>('deck_set_brightness', { epoch, brightness }),
  deckHostBrightness: () => invoke<number | null>('deck_host_brightness'),
  deckPollEvents: (epoch: number, after: number) => invoke<DeckEvents>('deck_poll_events', { epoch, after }),
  deckRelease: (epoch: number) => invoke<void>('deck_release', { epoch }),
};
