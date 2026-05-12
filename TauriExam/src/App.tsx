import { useEffect, useMemo, useRef, useState } from 'react';
import {
  BookOpen,
  Clock3,
  History,
  Moon,
  PlayCircle,
  RotateCcw,
  Settings,
  Sun,
} from 'lucide-react';
import { api } from './api';
import type {
  AiSettings,
  AiStreamEvent,
  AppPaths,
  BatchTranslateEvent,
  BankHealth,
  BankInfo,
  DragDropUserAnswer,
  HotspotUserAnswer,
  ExamAnswerDetail,
  ExamAnswerInput,
  ExamSessionSummary,
  PageImage,
  QuestionDetail,
  QuestionFlag,
  QuestionPracticeStats,
  QuestionSummary,
  InteractionModel,
  ReviewMode,
  TranslationRow,
} from './types';
import type { ExamCategory, TranslationMode } from './helpers';
import { viewTitle, isFlagged, flagTypesForQuestion, uniqueSorted, shuffle, expectedLetters, sameSet, matchesExamCategory, formatDuration } from './helpers';
import QuestionPanel from './QuestionPanel';
import QuestionList, { ReviewList, Stat } from './QuestionList';
import SettingsPanel from './SettingsPanel';
import { createCorrectDragAnswer, describeDragAnswer, gradeDragDrop, isDragDropComplete, normalizeDragAnswer } from './DragDropQuestion';
import { createCorrectHotspotAnswer, describeHotspotAnswer, gradeHotspot, isHotspotComplete, normalizeHotspotAnswer } from './HotspotQuestion';

type View = 'browse' | 'exam' | 'history' | 'review' | 'settings';
type Theme = 'light' | 'dark';

type ReturnContext = {
  view: 'history';
  sessionId: string;
  label: string;
};

type ExamDraft = {
  startedAt: number;
  questions: QuestionSummary[];
  index: number;
  answers: ExamAnswerInput[];
  selected: string[];
  dragAnswer: DragDropUserAnswer | null;
  hotspotAnswer: HotspotUserAnswer | null;
  questionStartedAt: number;
  finished: boolean;
};

function App() {
  const [theme, setTheme] = useState<Theme>(() => (localStorage.getItem('tauri-exam-theme') as Theme) || 'dark');
  const [view, setView] = useState<View>('browse');
  const [banks, setBanks] = useState<BankInfo[]>([]);
  const [bankId, setBankId] = useState('sc-100');
  const [questions, setQuestions] = useState<QuestionSummary[]>([]);
  const [selectedId, setSelectedId] = useState<string>('');
  const [detail, setDetail] = useState<QuestionDetail | null>(null);
  const [pages, setPages] = useState<PageImage[]>([]);
  const [pagesBusy, setPagesBusy] = useState(false);
  const [showAnswer, setShowAnswer] = useState(false);
  const [query, setQuery] = useState('');
  const [typeFilter, setTypeFilter] = useState('all');
  const [statusFilter, setStatusFilter] = useState('all');
  const [topicFilter, setTopicFilter] = useState('all');
  const [loading, setLoading] = useState('');
  const [error, setError] = useState('');
  const [examCount, setExamCount] = useState(20);
  const [examMode, setExamMode] = useState<'order' | 'random'>('order');
  const [examCategories, setExamCategories] = useState<ExamCategory[]>(['single_choice', 'multiple_choice', 'hotspot', 'drag_drop']);
  const [exam, setExam] = useState<ExamDraft | null>(null);
  const [history, setHistory] = useState<ExamSessionSummary[]>([]);
  const [expandedSessionId, setExpandedSessionId] = useState('');
  const [historyAnswers, setHistoryAnswers] = useState<Record<string, ExamAnswerDetail[]>>({});
  const [historyFilter, setHistoryFilter] = useState<'all' | 'wrong'>('all');
  const [appPaths, setAppPaths] = useState<AppPaths | null>(null);
  const [bankHealth, setBankHealth] = useState<BankHealth | null>(null);
  const [settingsMessage, setSettingsMessage] = useState('');
  const [flags, setFlags] = useState<QuestionFlag[]>([]);
  const [practiceStats, setPracticeStats] = useState<Record<string, QuestionPracticeStats>>({});
  const [reviewMode, setReviewMode] = useState<ReviewMode>('wrong');
  const [reviewQuestions, setReviewQuestions] = useState<QuestionSummary[]>([]);
  const [reviewSessionId, setReviewSessionId] = useState<string>('');
  const [returnContext, setReturnContext] = useState<ReturnContext | null>(null);
  const [interaction, setInteraction] = useState<InteractionModel | null>(null);
  const [aiSettings, setAiSettings] = useState<AiSettings | null>(null);
  const [aiPrompt, setAiPrompt] = useState('');
  const [aiAnswer, setAiAnswer] = useState('');
  const [aiBusy, setAiBusy] = useState(false);
  const [aiStreamChunkCount, setAiStreamChunkCount] = useState(0);
  const aiStreamHadDeltaRef = useRef(false);
  const selectedIdRef = useRef('');
  const [translationBusy, setTranslationBusy] = useState(false);
  const [translations, setTranslations] = useState<TranslationRow[]>([]);
  const [translationMode, setTranslationMode] = useState<TranslationMode>('original');
  const [translationLanguage, setTranslationLanguage] = useState('zh-CN');
  const [batchTranslationBusy, setBatchTranslationBusy] = useState(false);
  const [batchTranslation, setBatchTranslation] = useState<BatchTranslateEvent | null>(null);
  const [nowTick, setNowTick] = useState(Date.now());

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem('tauri-exam-theme', theme);
  }, [theme]);

  useEffect(() => {
    bootstrap();
  }, []);

  useEffect(() => {
    selectedIdRef.current = selectedId;
  }, [selectedId]);

  useEffect(() => {
    if (bankId) loadQuestions(bankId);
  }, [bankId]);

  useEffect(() => {
    if (selectedId) loadQuestion(bankId, selectedId);
  }, [selectedId]);

  useEffect(() => {
    if (view !== 'exam' || !exam) return;
    const id = window.setInterval(() => setNowTick(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [view, exam]);

  const filtered = useMemo(() => {
    const text = query.trim().toLowerCase();
    return questions.filter((question) => {
      const matchesText =
        !text ||
        question.id.toLowerCase().includes(text) ||
        String(question.sequence_number).includes(text) ||
        question.preview.toLowerCase().includes(text);
      const matchesType = typeFilter === 'all' || question.question_type === typeFilter;
      const matchesStatus = statusFilter === 'all' || question.status === statusFilter;
      const matchesTopic = topicFilter === 'all' || (question.topic || 'Uncategorized') === topicFilter;
      return matchesText && matchesType && matchesStatus && matchesTopic;
    });
  }, [questions, query, typeFilter, statusFilter, topicFilter]);

  const questionTypes = useMemo(() => uniqueSorted(questions.map((question) => question.question_type)), [questions]);
  const statuses = useMemo(() => uniqueSorted(questions.map((question) => question.status)), [questions]);
  const topics = useMemo(() => uniqueSorted(questions.map((question) => question.topic || 'Uncategorized')), [questions]);
  const examPool = useMemo(
    () => questions.filter((question) => examCategories.some((category) => matchesExamCategory(question.question_type, category))),
    [questions, examCategories],
  );
  const reviewFiltered = useMemo(() => {
    const text = query.trim().toLowerCase();
    return reviewQuestions.filter((question) => {
      const matchesText =
        !text ||
        question.id.toLowerCase().includes(text) ||
        String(question.sequence_number).includes(text) ||
        question.preview.toLowerCase().includes(text);
      const matchesTopic = topicFilter === 'all' || (question.topic || 'Uncategorized') === topicFilter;
      return matchesText && matchesTopic;
    });
  }, [reviewQuestions, query, topicFilter]);

  const reviewTopics = useMemo(() => uniqueSorted(reviewQuestions.map((question) => question.topic || 'Uncategorized')), [reviewQuestions]);
  const currentHistoryUserAnswer = useMemo(() => {
    if (!returnContext || !selectedId) return null;
    return historyAnswers[returnContext.sessionId]?.find((answer) => answer.question_id === selectedId)?.user_answer ?? null;
  }, [historyAnswers, returnContext, selectedId]);
  const isStructuredDrag = interaction?.kind === 'drag_drop' && interaction.can_auto_grade && interaction.slots.length > 0;
  const isStructuredHotspot = interaction?.kind === 'hotspot' && interaction.can_auto_grade && interaction.rows.length > 0 && interaction.options.length > 0;
  const currentDragAnswer = isStructuredDrag && interaction ? normalizeDragAnswer(interaction, exam?.dragAnswer) : null;
  const currentHotspotAnswer = isStructuredHotspot && interaction ? normalizeHotspotAnswer(interaction, exam?.hotspotAnswer) : null;
  const currentDragComplete = Boolean(isStructuredDrag && interaction && isDragDropComplete(interaction, currentDragAnswer));
  const currentHotspotComplete = Boolean(isStructuredHotspot && interaction && isHotspotComplete(interaction, currentHotspotAnswer));
  const isCurrentDragLike = Boolean(detail?.question_type.toLowerCase().includes('drag') || detail?.question_type.toLowerCase().includes('drop'));
  const isCurrentHotspotLike = Boolean(detail?.question_type.toLowerCase().includes('hotspot'));

  function isBankAvailable(candidateBankId: string) {
    return banks.some((bank) => bank.id === candidateBankId);
  }

  function missingBankMessage(candidateBankId: string) {
    return `这条记录属于题库 ${candidateBankId}，但当前题库目录里没有对应的 SQLite。缺 PDF 只会影响点击 PDF 按钮；缺 SQLite 无法读取题干。请把 ${candidateBankId}.sqlite 放回 question-banks 后刷新题库。`;
  }

  async function bootstrap() {
    try {
      setError('');
      setAppPaths(await api.getAppPaths());
      setAiSettings(await api.getAiSettings());
      const loadedBanks = await api.listBanks();
      setBanks(loadedBanks);
      if (loadedBanks[0]) setBankId(loadedBanks[0].id);
      await loadHistory();
    } catch (err) {
      setError(String(err));
    }
  }

  async function loadQuestions(nextBankId: string) {
    try {
      setLoading('正在读取题库...');
      setError('');
      const loaded = await api.listQuestions(nextBankId);
      setQuestions(loaded);
      const nextSelectedId = loaded.some((question) => question.id === selectedId) ? selectedId : loaded[0]?.id || '';
      setSelectedId(nextSelectedId);
      if (!nextSelectedId) {
        setDetail(null);
        setInteraction(null);
        setTranslations([]);
        setPages([]);
      } else if (nextSelectedId === selectedId) {
        await loadQuestion(nextBankId, nextSelectedId);
      }
      setBankHealth(null);
      setFlags(await api.listQuestionFlags(nextBankId));
      await refreshPracticeStats(nextBankId, loaded.map((question) => question.id));
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading('');
    }
  }

  async function loadQuestion(nextBankId: string, questionId: string) {
    try {
      setLoading('正在读取题目...');
      setError('');
      setInteraction(null);
      const loaded = await api.getQuestion(nextBankId, questionId);
      setDetail(loaded);
      setInteraction(await api.getInteractionModel(nextBankId, questionId));
      setTranslations(await api.getCachedTranslations(nextBankId, questionId, translationLanguage));
      setPages([]);
      setShowAnswer(false);
      setAiAnswer('');
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading('');
    }
  }

  async function togglePages() {
    if (!detail || pagesBusy) return;
    if (pages.length > 0) {
      setPages([]);
      return;
    }
    const questionId = detail.id;
    try {
      setPagesBusy(true);
      setLoading('正在加载 PDF 页...');
      setError('');
      const loadedPages = await api.getSourcePages(bankId, questionId);
      if (selectedId === questionId) {
        setPages(loadedPages);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setPagesBusy(false);
      setLoading('');
    }
  }

  async function loadHistory() {
    try {
      setHistory(await api.listExamSessions());
    } catch {
      setHistory([]);
    }
  }

  async function loadReview(nextMode = reviewMode, sessionId = reviewSessionId) {
    try {
      setLoading('正在读取复习题...');
      setError('');
      const effectiveBankId = banks.length > 0 && !isBankAvailable(bankId) ? banks[0].id : bankId;
      if (effectiveBankId !== bankId) {
        setBankId(effectiveBankId);
      }
      const loaded = await api.listReviewQuestions(effectiveBankId, nextMode, sessionId || undefined);
      setReviewQuestions(loaded);
      if (loaded[0]) setSelectedId(loaded[0].id);
      await refreshPracticeStats(effectiveBankId, loaded.map((question) => question.id), true);
      setView('review');
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading('');
    }
  }

  async function refreshPracticeStats(nextBankId: string, questionIds: string[], merge = false) {
    const uniqueIds = [...new Set(questionIds.filter(Boolean))];
    if (uniqueIds.length === 0) {
      if (!merge) setPracticeStats({});
      return;
    }
    const stats = await api.getQuestionPracticeStats(nextBankId, uniqueIds);
    const mapped = Object.fromEntries(stats.map((item) => [item.question_id, item]));
    setPracticeStats((current) => (merge ? { ...current, ...mapped } : mapped));
  }

  async function toggleQuestionFlag(flagType: string) {
    if (!detail) return;
    try {
      setError('');
      const enabled = !isFlagged(flags, detail.id, flagType);
      const updated = await api.setQuestionFlag({
        bank_id: bankId,
        question_id: detail.id,
        flag_type: flagType,
        enabled,
        note: null,
      });
      setFlags(updated);
      if (view === 'review') {
        setReviewQuestions(await api.listReviewQuestions(bankId, reviewMode, reviewSessionId || undefined));
      }
    } catch (err) {
      setError(String(err));
    }
  }

  async function refreshQuestionBanks() {
    try {
      setLoading('正在刷新题库目录...');
      setError('');
      setSettingsMessage('');
      const loadedBanks = await api.refreshBanks();
      setBanks(loadedBanks);
      if (loadedBanks.length === 0) {
        setQuestions([]);
        setSelectedId('');
        setDetail(null);
        setSettingsMessage('未发现题库，请把同名 SQLite/PDF 放入题库目录后再次刷新。');
        return;
      }
      const nextBankId = loadedBanks.some((bank) => bank.id === bankId) ? bankId : loadedBanks[0].id;
      setBankId(nextBankId);
      await loadQuestions(nextBankId);
      setSettingsMessage(`已刷新，发现 ${loadedBanks.length} 个题库。`);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading('');
    }
  }

  async function checkCurrentBankHealth() {
    if (!bankId) return;
    try {
      setLoading('正在检查题库...');
      setError('');
      setBankHealth(await api.checkBankHealth(bankId));
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading('');
    }
  }

  async function openDataDir() {
    try {
      await api.openDataDir();
    } catch (err) {
      setError(String(err));
    }
  }

  async function openQuestionBanksDir() {
    try {
      await api.openQuestionBanksDir();
    } catch (err) {
      setError(String(err));
    }
  }

  async function saveAiSettings(next: AiSettings) {
    try {
      setError('');
      setAiSettings(await api.saveAiSettings(next));
      setSettingsMessage('AI 设置已保存。');
    } catch (err) {
      setError(String(err));
    }
  }

  async function testTranslatorSettings(settings: AiSettings) {
    try {
      setError('');
      setSettingsMessage('正在用当前表单里的 Microsoft Translator 配置测试：Hello → 中文...');
      const result = await api.testTranslatorSettings(settings);
      setSettingsMessage(`Microsoft Translator 测试成功：${result.source_text} → ${result.translated_text}`);
    } catch (err) {
      setSettingsMessage('');
      setError(String(err));
    }
  }

  async function askAi(actionType: 'analyze' | 'summarize' | 'freeform' = 'analyze') {
    if (!detail) return;
    const questionId = detail.id;
    try {
      setAiBusy(true);
      setError('');
      setAiAnswer('');
      setAiStreamChunkCount(0);
      aiStreamHadDeltaRef.current = false;
      const streamChannel = api.createAiStreamChannel((payload: AiStreamEvent) => {
        if (payload.question_id !== selectedIdRef.current) return;
        if (payload.error) {
          setError(payload.error);
          setAiBusy(false);
          return;
        }
        if (payload.delta) {
          aiStreamHadDeltaRef.current = true;
          setAiStreamChunkCount((current) => current + 1);
          setAiAnswer((current) => current + payload.delta);
        }
        if (payload.done) {
          setAiBusy(false);
        }
      });
      const response = await api.askAiAboutQuestionStream(
        {
          bank_id: bankId,
          question_id: questionId,
          user_prompt: aiPrompt || null,
          action_type: actionType,
        },
        streamChannel,
      );
      if (response.content && !aiStreamHadDeltaRef.current && selectedIdRef.current === questionId) {
        setAiAnswer(response.content);
      }
    } catch (err) {
      setError(String(err));
      setAiBusy(false);
    } finally {
      // 正常结束由 ai-stream done 事件关闭 busy；这里兜底避免事件丢失。
      setAiBusy(false);
    }
  }

  async function translateCurrentQuestion(force: boolean) {
    if (!detail) return;
    try {
      setTranslationBusy(true);
      setError('');
      const rows = await api.translateQuestion({
        bank_id: bankId,
        question_id: detail.id,
        language: translationLanguage,
        force,
      });
      setTranslations(rows);
      setTranslationMode('translated');
    } catch (err) {
      setError(String(err));
    } finally {
      setTranslationBusy(false);
    }
  }

  async function batchTranslateCurrentBank() {
    if (!bankId || batchTranslationBusy) return;
    try {
      setBatchTranslationBusy(true);
      setError('');
      setSettingsMessage('正在批量翻译题库，将按题号顺序逐题处理；已存在的翻译会自动跳过。');
      const channel = api.createBatchTranslateChannel((event: BatchTranslateEvent) => {
        setBatchTranslation(event);
        if (event.error) setError(event.error);
        if (event.done) setBatchTranslationBusy(false);
      });
      const result = await api.batchTranslateBank(
        {
          bank_id: bankId,
          language: translationLanguage,
          force: false,
        },
        channel,
      );
      setSettingsMessage(`批量翻译完成：新翻译 ${result.translated} 题，跳过 ${result.skipped} 题。翻译库：${result.translation_db_path}`);
      if (detail) {
        setTranslations(await api.getCachedTranslations(bankId, detail.id, translationLanguage));
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setBatchTranslationBusy(false);
    }
  }

  function startExam() {
    const pool = [...examPool];
    const picked = examMode === 'random' ? shuffle(pool).slice(0, examCount) : pool.slice(0, examCount);
    if (!picked.length) return;
    setExam({
      startedAt: Date.now(),
      questions: picked,
      index: 0,
      answers: [],
      selected: [],
      dragAnswer: null,
      hotspotAnswer: null,
      questionStartedAt: Date.now(),
      finished: false,
    });
    setSelectedId(picked[0].id);
    setView('exam');
  }

  async function submitExamAnswer(manualCorrect?: boolean | null) {
    if (!exam || !detail) return;
    setError('');
    const recommended = detail.recommended_answer || '';
    const expected = expectedLetters(recommended);
    let userAnswer = exam.selected.join(',');
    let correctAnswer = detail.source_answer || '';
    let recommendedAnswer = recommended;
    let isCorrect: boolean | null = manualCorrect ?? null;

    if (isStructuredDrag && interaction) {
      const dragAnswer = normalizeDragAnswer(interaction, exam.dragAnswer);
      if (!isDragDropComplete(interaction, dragAnswer)) {
        setError('这道 Drag Drop 题还有槽位未填，请填完后再提交。');
        return;
      }
      userAnswer = JSON.stringify(dragAnswer);
      correctAnswer = JSON.stringify(createCorrectDragAnswer(interaction));
      recommendedAnswer = describeDragAnswer(interaction, createCorrectDragAnswer(interaction));
      isCorrect = gradeDragDrop(interaction, dragAnswer);
    } else if (isStructuredHotspot && interaction) {
      const hotspotAnswer = normalizeHotspotAnswer(interaction, exam.hotspotAnswer);
      if (!isHotspotComplete(interaction, hotspotAnswer)) {
        setError('这道 Hotspot 题还有下拉框未选择，请填完后再提交。');
        return;
      }
      userAnswer = JSON.stringify(hotspotAnswer);
      correctAnswer = JSON.stringify(createCorrectHotspotAnswer(interaction));
      recommendedAnswer = describeHotspotAnswer(interaction, createCorrectHotspotAnswer(interaction));
      isCorrect = gradeHotspot(interaction, hotspotAnswer);
    } else {
      const isAuto = detail.options.length > 0 && expected.length > 0;
      isCorrect = isAuto ? sameSet(exam.selected, expected) : manualCorrect ?? null;
      userAnswer = userAnswer || (manualCorrect === true ? 'manual:correct' : manualCorrect === false ? 'manual:wrong' : '');
    }

    const answer: ExamAnswerInput = {
      question_id: detail.id,
      sequence_number: detail.sequence_number,
      user_answer: userAnswer,
      correct_answer: correctAnswer,
      recommended_answer: recommendedAnswer,
      is_correct: isCorrect,
      duration_seconds: Math.max(1, Math.round((Date.now() - exam.questionStartedAt) / 1000)),
    };
    const nextAnswers = [...exam.answers, answer];
    const nextIndex = exam.index + 1;
    if (nextIndex >= exam.questions.length) {
      const duration = Math.max(1, Math.round((Date.now() - exam.startedAt) / 1000));
      await api.saveExamResult({
        bank_id: bankId,
        title: `${banks.find((bank) => bank.id === bankId)?.name || bankId} ${new Date().toLocaleString()}`,
        mode: `${examMode}:${examCategories.join(',')}`,
        duration_seconds: duration,
        answers: nextAnswers,
      });
      setExam({ ...exam, answers: nextAnswers, finished: true });
      await refreshPracticeStats(bankId, exam.questions.map((question) => question.id), true);
      await loadHistory();
      setView('history');
      return;
    }
    const nextQuestion = exam.questions[nextIndex];
    setExam({
      ...exam,
      index: nextIndex,
      answers: nextAnswers,
      selected: [],
      dragAnswer: null,
      hotspotAnswer: null,
      questionStartedAt: Date.now(),
    });
    setSelectedId(nextQuestion.id);
  }

  function updateDragAnswer(value: DragDropUserAnswer) {
    setExam((current) => (current ? { ...current, dragAnswer: value } : current));
  }

  function updateHotspotAnswer(value: HotspotUserAnswer) {
    setExam((current) => (current ? { ...current, hotspotAnswer: value } : current));
  }

  function toggleChoice(key: string) {
    setExam((current) => {
      if (!current) return current;
      if (detail?.question_type.toLowerCase().includes('single_choice')) {
        return { ...current, selected: [key] };
      }
      const exists = current.selected.includes(key);
      const selected = exists ? current.selected.filter((item) => item !== key) : [...current.selected, key];
      return { ...current, selected };
    });
  }

  function toggleExamCategory(category: ExamCategory) {
    setExamCategories((current) => {
      const next = current.includes(category) ? current.filter((item) => item !== category) : [...current, category];
      return next.length ? next : current;
    });
  }

  async function toggleHistorySession(sessionId: string) {
    if (expandedSessionId === sessionId) {
      setExpandedSessionId('');
      return;
    }
    setExpandedSessionId(sessionId);
    if (!historyAnswers[sessionId]) {
      setHistoryAnswers((current) => ({ ...current, [sessionId]: [] }));
      const answers = await api.listExamAnswers(sessionId);
      setHistoryAnswers((current) => ({ ...current, [sessionId]: answers }));
    }
  }

  function openHistoryQuestion(answer: ExamAnswerDetail) {
    if (!isBankAvailable(answer.bank_id)) {
      setError(missingBankMessage(answer.bank_id));
      return;
    }
    setError('');
    setBankId(answer.bank_id);
    setSelectedId(answer.question_id);
    setReturnContext({ view: 'history', sessionId: answer.session_id, label: '返回本次考试记录' });
    setView('browse');
  }

  async function returnToHistory() {
    if (!returnContext) return;
    setView('history');
    await toggleHistorySession(returnContext.sessionId);
    setExpandedSessionId(returnContext.sessionId);
    setReturnContext(null);
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">TE</div>
          <div>
            <h1>TauriExam</h1>
            <p>本地题库考试工具</p>
          </div>
        </div>

        <nav className="nav">
          <button className={view === 'browse' ? 'active' : ''} onClick={() => setView('browse')}>
            <BookOpen size={18} /> 题库
          </button>
          <button className={view === 'exam' ? 'active' : ''} onClick={() => setView('exam')}>
            <PlayCircle size={18} /> 考试
          </button>
          <button className={view === 'history' ? 'active' : ''} onClick={() => setView('history')}>
            <History size={18} /> 历史
          </button>
          <button className={view === 'review' ? 'active' : ''} onClick={() => loadReview()}>
            <RotateCcw size={18} /> 复习
          </button>
          <button className={view === 'settings' ? 'active' : ''} onClick={() => setView('settings')}>
            <Settings size={18} /> 控制面板
          </button>
        </nav>

        <div className="sidebar-card">
          <label>题库</label>
          <select value={bankId} onChange={(event) => setBankId(event.target.value)}>
            {banks.map((bank) => (
              <option key={bank.id} value={bank.id}>
                {bank.name} ({bank.question_count})
              </option>
            ))}
          </select>
        </div>

        <button className="theme-toggle" onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}>
          {theme === 'dark' ? <Sun size={18} /> : <Moon size={18} />}
          {theme === 'dark' ? '浅色主题' : '深色主题'}
        </button>
      </aside>

      <main className="main">
        <header className="topbar">
          <div>
            <h2>{viewTitle(view)}</h2>
            <p>{loading || 'SQLite 题库只读，考试记录写入独立学习库。'}</p>
          </div>
          <div className="stats-row">
            <Stat label="题目" value={questions.length} />
            <Stat label="类型" value={questionTypes.length} />
            <Stat label="历史" value={history.length} />
          </div>
        </header>

        {error && <div className="error-box">{error}</div>}

        {view === 'browse' && (
          <section className="content-grid">
            <QuestionList
              questions={filtered}
              allCount={questions.length}
              selectedId={selectedId}
              query={query}
              typeFilter={typeFilter}
              statusFilter={statusFilter}
              types={questionTypes}
              statuses={statuses}
              topics={topics}
              topicFilter={topicFilter}
              practiceStats={practiceStats}
              onQuery={setQuery}
              onType={setTypeFilter}
              onStatus={setStatusFilter}
              onTopic={setTopicFilter}
              onSelect={setSelectedId}
            />
            <QuestionPanel
              detail={detail}
              pages={pages}
              showAnswer={showAnswer}
              flags={detail ? flagTypesForQuestion(flags, detail.id) : []}
              interaction={interaction}
              translations={translations}
              translationMode={translationMode}
              translationLanguage={translationLanguage}
              aiEnabled={aiSettings?.enabled ?? false}
              aiPrompt={aiPrompt}
              aiAnswer={aiAnswer}
              aiBusy={aiBusy}
              aiStreamChunkCount={aiStreamChunkCount}
              translationBusy={translationBusy}
              pagesBusy={pagesBusy}
              returnLabel={returnContext?.label}
              historyUserAnswer={currentHistoryUserAnswer}
              onReturn={returnContext ? returnToHistory : undefined}
              onToggleFlag={toggleQuestionFlag}
              onAiPrompt={setAiPrompt}
              onAskAi={askAi}
              onTranslationMode={setTranslationMode}
              onTranslationLanguage={setTranslationLanguage}
              onTranslate={translateCurrentQuestion}
              onToggleAnswer={() => setShowAnswer((value) => !value)}
              onTogglePages={togglePages}
              onPrev={(() => { const idx = filtered.findIndex(q => q.id === selectedId); return idx > 0 ? () => setSelectedId(filtered[idx - 1].id) : undefined; })()}
              onNext={(() => { const idx = filtered.findIndex(q => q.id === selectedId); return idx >= 0 && idx < filtered.length - 1 ? () => setSelectedId(filtered[idx + 1].id) : undefined; })()}
            />
          </section>
        )}

        {view === 'exam' && (
          <section className="exam-layout">
            <div className="panel setup-panel">
              <h3>创建考试</h3>
              <label>出题数量</label>
              <input
                type="number"
                min={1}
                max={examPool.length || 1}
                value={examCount}
                onChange={(event) => setExamCount(Number(event.target.value))}
              />
              <label>出题方式</label>
              <select value={examMode} onChange={(event) => setExamMode(event.target.value as 'order' | 'random')}>
                <option value="order">按题号顺序</option>
                <option value="random">随机</option>
              </select>
              <label>出题类型</label>
              <div className="category-grid">
                <button className={examCategories.includes('single_choice') ? 'selected' : ''} onClick={() => toggleExamCategory('single_choice')}>
                  单选
                </button>
                <button className={examCategories.includes('multiple_choice') ? 'selected' : ''} onClick={() => toggleExamCategory('multiple_choice')}>
                  多选
                </button>
                <button className={examCategories.includes('hotspot') ? 'selected' : ''} onClick={() => toggleExamCategory('hotspot')}>
                  Hotspot
                </button>
                <button className={examCategories.includes('drag_drop') ? 'selected' : ''} onClick={() => toggleExamCategory('drag_drop')}>
                  Drag
                </button>
              </div>
              <button className="primary" onClick={startExam}>
                开始考试
              </button>
              <p className="muted">当前题池：{examPool.length} 题。随机模式会在所选类型内随机。</p>
            </div>

            <div className="panel exam-panel">
              {exam && detail ? (
                <>
                  <div className="exam-head">
                    <strong>
                      第 {exam.index + 1} / {exam.questions.length} 题
                    </strong>
                    <span>
                      <Clock3 size={16} /> {Math.round((nowTick - exam.questionStartedAt) / 1000)}s
                    </span>
                    {practiceStats[detail.id]?.wrong_count > 0 && <span className="wrong-count-pill">历史错过 {practiceStats[detail.id].wrong_count} 次</span>}
                  </div>
                  <QuestionPanel
                    detail={detail}
                    pages={[]}
                    showAnswer={false}
                    onToggleAnswer={() => undefined}
                    onTogglePages={() => undefined}
                    compact
                    translationToolbarOnly
                    hideActions
                    hideAnswerAreas
                    pagesBusy={pagesBusy}
                    interaction={interaction}
                    translations={translations}
                    translationMode={translationMode}
                    translationLanguage={translationLanguage}
                    translationBusy={translationBusy}
                    selectedOptions={exam.selected}
                    dragAnswer={currentDragAnswer}
                    hotspotAnswer={currentHotspotAnswer}
                    onTranslationMode={setTranslationMode}
                    onTranslationLanguage={setTranslationLanguage}
                    onTranslate={translateCurrentQuestion}
                    onOptionSelect={toggleChoice}
                    onDragAnswerChange={updateDragAnswer}
                    onHotspotAnswerChange={updateHotspotAnswer}
                  />
                  {isCurrentDragLike && !interaction ? (
                    <div className="choice-bar">
                      <button className="primary" disabled>
                        正在加载 Drag Drop 交互模型...
                      </button>
                    </div>
                  ) : isCurrentHotspotLike && !interaction ? (
                    <div className="choice-bar">
                      <button className="primary" disabled>
                        正在加载 Hotspot 交互模型...
                      </button>
                    </div>
                  ) : isStructuredDrag ? (
                    <div className="choice-bar">
                      <button className="primary" disabled={!currentDragComplete} onClick={() => submitExamAnswer()}>
                        提交 Drag Drop
                      </button>
                      {!currentDragComplete && <span className="muted">请先填完所有槽位。</span>}
                    </div>
                  ) : isStructuredHotspot ? (
                    <div className="choice-bar">
                      <button className="primary" disabled={!currentHotspotComplete} onClick={() => submitExamAnswer()}>
                        提交 Hotspot
                      </button>
                      {!currentHotspotComplete && <span className="muted">请先完成所有下拉选择。</span>}
                    </div>
                  ) : detail.options.length > 0 ? (
                    <div className="choice-bar">
                      <button className="primary" disabled={exam.selected.length === 0} onClick={() => submitExamAnswer()}>
                        提交本题
                      </button>
                    </div>
                  ) : (
                    <div className="choice-bar">
                      <button className="correct" onClick={() => submitExamAnswer(true)}>
                        我答对了
                      </button>
                      <button className="wrong" onClick={() => submitExamAnswer(false)}>
                        我答错了
                      </button>
                    </div>
                  )}
                </>
              ) : (
                <div className="empty-state">设置题数和类型后点击开始考试。</div>
              )}
            </div>
          </section>
        )}

        {view === 'history' && (
          <section className="panel history-panel">
            <h3>历史考试记录</h3>
            {history.length === 0 ? (
              <div className="empty-state">还没有考试记录。</div>
            ) : (
              <div className="history-list">
                {history.map((item) => (
                  <div className="history-card" key={item.id}>
                    <button className="history-item" onClick={() => toggleHistorySession(item.id)}>
                      <div>
                        <strong>{item.title}</strong>
                        <p>
                          {new Date(item.finished_at).toLocaleString()} · {item.total_questions} 题
                        </p>
                      </div>
                      <div className="history-score">
                        <span className="ok">{item.correct_count} 对</span>
                        <span className="bad">{item.wrong_count} 错</span>
                        <span>{formatDuration(item.duration_seconds)}</span>
                      </div>
                    </button>
                    {expandedSessionId === item.id && (
                      <div className="history-answer-list">
                        {(historyAnswers[item.id] || []).length === 0 ? (
                          <div className="muted">正在读取本次考试题目...</div>
                        ) : (
                          <>
                            <div className="history-filter-bar">
                              <button className={historyFilter === 'all' ? 'active' : ''} onClick={() => setHistoryFilter('all')}>全部显示</button>
                              <button className={historyFilter === 'wrong' ? 'active' : ''} onClick={() => setHistoryFilter('wrong')}>只看错题</button>
                            </div>
                            {historyAnswers[item.id]
                              .filter((a) => historyFilter === 'all' || a.is_correct === false)
                              .map((answer, index) => (
                              <button className="history-answer-row" key={answer.id} onClick={() => openHistoryQuestion(answer)}>
                                <span>第 {index + 1} 题 / 原题 Q{answer.sequence_number}</span>
                                <span>{answer.user_answer || '未作答'}</span>
                                <span>{formatDuration(answer.duration_seconds)}</span>
                                <span>{answer.is_correct === false && practiceStats[answer.question_id]?.wrong_count > 0 ? `错 ${practiceStats[answer.question_id].wrong_count}` : ''}</span>
                                <strong className={answer.is_correct ? 'ok' : answer.is_correct === false ? 'bad' : ''}>
                                  {answer.is_correct ? '正确' : answer.is_correct === false ? '错误' : '复核'}
                                </strong>
                              </button>
                            ))}
                          </>
                        )}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </section>
        )}

        {view === 'review' && (
          <section className="content-grid">
            <ReviewList
              mode={reviewMode}
              questions={reviewFiltered}
              allCount={reviewQuestions.length}
              selectedId={selectedId}
              query={query}
              sessions={history.filter(h => h.bank_id === bankId && h.wrong_count > 0)}
              sessionId={reviewSessionId}
              topics={reviewTopics}
              topicFilter={topicFilter}
              practiceStats={practiceStats}
              onQuery={setQuery}
              onTopic={setTopicFilter}
              onMode={(mode) => {
                setReviewMode(mode);
                setReviewSessionId('');
                loadReview(mode, '');
              }}
              onSessionChange={(sid) => {
                setReviewSessionId(sid);
                loadReview(reviewMode, sid);
              }}
              onSelect={setSelectedId}
            />
            <QuestionPanel
              detail={detail}
              pages={pages}
              showAnswer={showAnswer}
              flags={detail ? flagTypesForQuestion(flags, detail.id) : []}
              interaction={interaction}
              translations={translations}
              translationMode={translationMode}
              translationLanguage={translationLanguage}
              aiEnabled={aiSettings?.enabled ?? false}
              aiPrompt={aiPrompt}
              aiAnswer={aiAnswer}
              aiBusy={aiBusy}
              aiStreamChunkCount={aiStreamChunkCount}
              translationBusy={translationBusy}
              pagesBusy={pagesBusy}
              onToggleFlag={toggleQuestionFlag}
              onAiPrompt={setAiPrompt}
              onAskAi={askAi}
              onTranslationMode={setTranslationMode}
              onTranslationLanguage={setTranslationLanguage}
              onTranslate={translateCurrentQuestion}
              onToggleAnswer={() => setShowAnswer((value) => !value)}
              onTogglePages={togglePages}
              onPrev={(() => { const idx = reviewFiltered.findIndex(q => q.id === selectedId); return idx > 0 ? () => setSelectedId(reviewFiltered[idx - 1].id) : undefined; })()}
              onNext={(() => { const idx = reviewFiltered.findIndex(q => q.id === selectedId); return idx >= 0 && idx < reviewFiltered.length - 1 ? () => setSelectedId(reviewFiltered[idx + 1].id) : undefined; })()}
            />
          </section>
        )}

        {view === 'settings' && (
          <SettingsPanel
            appPaths={appPaths}
            banks={banks}
            bankId={bankId}
            bankHealth={bankHealth}
            aiSettings={aiSettings}
            message={settingsMessage}
            batchTranslation={batchTranslation}
            batchTranslationBusy={batchTranslationBusy}
            onOpenDataDir={openDataDir}
            onOpenQuestionBanksDir={openQuestionBanksDir}
            onRefreshBanks={refreshQuestionBanks}
            onCheckHealth={checkCurrentBankHealth}
            onSaveAiSettings={saveAiSettings}
            onTestTranslatorSettings={testTranslatorSettings}
            onBatchTranslate={batchTranslateCurrentBank}
          />
        )}
      </main>
    </div>
  );
}

export default App;
