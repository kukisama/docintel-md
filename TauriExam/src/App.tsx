import { useEffect, useMemo, useRef, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import {
  ArrowDown,
  ArrowUp,
  BookOpen,
  Clock3,
  FileText,
  FolderOpen,
  History,
  Moon,
  PlayCircle,
  RefreshCw,
  RotateCcw,
  Search,
  Settings,
  ShieldCheck,
  Star,
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
  ExamAnswerDetail,
  ExamAnswerInput,
  ExamSessionSummary,
  PageImage,
  QuestionDetail,
  QuestionFlag,
  QuestionSummary,
  InteractionModel,
  ReviewMode,
  TranslationRow,
} from './types';

type View = 'browse' | 'exam' | 'history' | 'review' | 'settings';
type Theme = 'light' | 'dark';
type ExamCategory = 'single_choice' | 'multiple_choice' | 'hotspot';
type TranslationMode = 'original' | 'translated' | 'side_by_side';

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
  const [loading, setLoading] = useState('');
  const [error, setError] = useState('');
  const [examCount, setExamCount] = useState(20);
  const [examMode, setExamMode] = useState<'order' | 'random'>('order');
  const [examCategories, setExamCategories] = useState<ExamCategory[]>(['single_choice', 'multiple_choice', 'hotspot']);
  const [exam, setExam] = useState<ExamDraft | null>(null);
  const [history, setHistory] = useState<ExamSessionSummary[]>([]);
  const [expandedSessionId, setExpandedSessionId] = useState('');
  const [historyAnswers, setHistoryAnswers] = useState<Record<string, ExamAnswerDetail[]>>({});
  const [historyFilter, setHistoryFilter] = useState<'all' | 'wrong'>('all');
  const [appPaths, setAppPaths] = useState<AppPaths | null>(null);
  const [bankHealth, setBankHealth] = useState<BankHealth | null>(null);
  const [settingsMessage, setSettingsMessage] = useState('');
  const [flags, setFlags] = useState<QuestionFlag[]>([]);
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
    if (selectedId) loadQuestion(selectedId);
  }, [selectedId]);

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
      return matchesText && matchesType && matchesStatus;
    });
  }, [questions, query, typeFilter, statusFilter]);

  const questionTypes = useMemo(() => uniqueSorted(questions.map((question) => question.question_type)), [questions]);
  const statuses = useMemo(() => uniqueSorted(questions.map((question) => question.status)), [questions]);
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
      return matchesText;
    });
  }, [reviewQuestions, query]);

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
      setSelectedId((current) => (loaded.some((question) => question.id === current) ? current : loaded[0]?.id || ''));
      setBankHealth(null);
      setFlags(await api.listQuestionFlags(nextBankId));
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading('');
    }
  }

  async function loadQuestion(questionId: string) {
    try {
      setLoading('正在读取题目...');
      setError('');
      const loaded = await api.getQuestion(bankId, questionId);
      setDetail(loaded);
      setInteraction(await api.getInteractionModel(bankId, questionId));
      setTranslations(await api.getCachedTranslations(bankId, questionId, translationLanguage));
      setPages([]);
      setShowAnswer(false);
      setAiAnswer('');
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading('');
    }
  }

  async function loadPages() {
    if (!detail || pagesBusy) return;
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
      const loaded = await api.listReviewQuestions(bankId, nextMode, sessionId || undefined);
      setReviewQuestions(loaded);
      if (loaded[0]) setSelectedId(loaded[0].id);
      setView('review');
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading('');
    }
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
        setReviewQuestions(await api.listReviewQuestions(bankId, reviewMode));
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
      questionStartedAt: Date.now(),
      finished: false,
    });
    setSelectedId(picked[0].id);
    setView('exam');
  }

  async function submitExamAnswer(manualCorrect?: boolean | null) {
    if (!exam || !detail) return;
    const selected = exam.selected.join(',');
    const recommended = detail.recommended_answer || '';
    const expected = expectedLetters(recommended);
    const isAuto = detail.options.length > 0 && expected.length > 0;
    const isCorrect = isAuto ? sameSet(exam.selected, expected) : manualCorrect ?? null;
    const answer: ExamAnswerInput = {
      question_id: detail.id,
      sequence_number: detail.sequence_number,
      user_answer: selected || (manualCorrect === true ? 'manual:correct' : manualCorrect === false ? 'manual:wrong' : ''),
      correct_answer: detail.source_answer || '',
      recommended_answer: recommended,
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
      questionStartedAt: Date.now(),
    });
    setSelectedId(nextQuestion.id);
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
              onQuery={setQuery}
              onType={setTypeFilter}
              onStatus={setStatusFilter}
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
              onReturn={returnContext ? returnToHistory : undefined}
              onToggleFlag={toggleQuestionFlag}
              onAiPrompt={setAiPrompt}
              onAskAi={askAi}
              onTranslationMode={setTranslationMode}
              onTranslationLanguage={setTranslationLanguage}
              onTranslate={translateCurrentQuestion}
              onToggleAnswer={() => setShowAnswer((value) => !value)}
              onLoadPages={loadPages}
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
                      <Clock3 size={16} /> {Math.round((Date.now() - exam.questionStartedAt) / 1000)}s
                    </span>
                  </div>
                  <QuestionPanel
                    detail={detail}
                    pages={[]}
                    showAnswer={false}
                    onToggleAnswer={() => undefined}
                    onLoadPages={() => undefined}
                    compact
                    hideActions
                    hideAnswerAreas
                    pagesBusy={pagesBusy}
                    selectedOptions={exam.selected}
                    onOptionSelect={toggleChoice}
                  />
                  {detail.options.length > 0 ? (
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
              onQuery={setQuery}
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
              onLoadPages={loadPages}
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

type SettingsTab = 'files' | 'ai' | 'translation';

function SettingsPanel(props: {
  appPaths: AppPaths | null;
  banks: BankInfo[];
  bankId: string;
  bankHealth: BankHealth | null;
  aiSettings: AiSettings | null;
  message: string;
  batchTranslation: BatchTranslateEvent | null;
  batchTranslationBusy: boolean;
  onOpenDataDir: () => void;
  onOpenQuestionBanksDir: () => void;
  onRefreshBanks: () => void;
  onCheckHealth: () => void;
  onSaveAiSettings: (settings: AiSettings) => void;
  onTestTranslatorSettings: (settings: AiSettings) => void;
  onBatchTranslate: () => void;
}) {
  const currentBank = props.banks.find((bank) => bank.id === props.bankId);
  const [tab, setTab] = useState<SettingsTab>('files');
  const [draft, setDraft] = useState<AiSettings>(
    props.aiSettings || {
      enabled: false,
      base_url: 'https://api.openai.com/v1',
      api_version: '',
      api_key: '',
      model: 'gpt-4.1-mini',
      temperature: 0.7,
      system_prompt: '',
      prompt_analyze: '',
      prompt_summarize: '',
      translation_provider: 'ai',
      translator_endpoint: 'https://api.cognitive.microsofttranslator.com',
      translator_key: '',
      translator_region: '',
    },
  );

  useEffect(() => {
    if (props.aiSettings) setDraft(props.aiSettings);
  }, [props.aiSettings]);

  const batchProgressPercent = props.batchTranslation?.total
    ? Math.round((props.batchTranslation.current_index / props.batchTranslation.total) * 100)
    : 0;

  return (
    <section className="settings-sheet">
      <div className="settings-tabs">
        <button className={tab === 'files' ? 'active' : ''} onClick={() => setTab('files')}>
          <FolderOpen size={16} /> 文件与题库
        </button>
        <button className={tab === 'ai' ? 'active' : ''} onClick={() => setTab('ai')}>
          <Settings size={16} /> AI 接口
        </button>
        <button className={tab === 'translation' ? 'active' : ''} onClick={() => setTab('translation')}>
          <BookOpen size={16} /> 翻译服务
        </button>
      </div>

      {tab === 'files' && (
        <div className="settings-tab-body">
          <div className="panel settings-card">
            <div className="settings-head">
              <h3>架构文件</h3>
              <div className="action-row compact-actions">
                <button onClick={props.onOpenDataDir}><FolderOpen size={16} /> 数据目录</button>
                <button onClick={props.onOpenQuestionBanksDir}><FolderOpen size={16} /> 题库目录</button>
                <button className="primary" onClick={props.onRefreshBanks}><RefreshCw size={16} /> 刷新</button>
              </div>
            </div>
            <table className="file-table">
              <tbody>
                <tr><td className="ft-label">应用数据</td><td><code>{props.appPaths?.data_dir || '...'}</code></td></tr>
                <tr><td className="ft-label">学习数据库</td><td><code>{props.appPaths?.app_db_path || '...'}</code></td></tr>
                <tr><td className="ft-label">题库目录</td><td><code>{props.appPaths?.question_banks_dir || '...'}</code></td></tr>
                <tr><td className="ft-label">PDF 缓存</td><td><code>{props.appPaths?.page_cache_dir || '...'}</code></td></tr>
                {props.bankHealth?.translation_db_path && (
                  <tr><td className="ft-label">翻译库</td><td><code>{props.bankHealth.translation_db_path}</code></td></tr>
                )}
              </tbody>
            </table>
          </div>

          <div className="settings-row-pair">
            <div className="panel settings-card">
              <div className="settings-head">
                <h3>题库</h3>
              </div>
              {props.banks.length === 0 ? (
                <div className="empty-state small">未识别到题库。</div>
              ) : (
                <div className="bank-mini-list">
                  {props.banks.map((bank) => (
                    <div className="bank-mini-row" key={bank.id}>
                      <strong>{bank.name}</strong>
                      <span>{bank.question_count} 题</span>
                      <em>{bank.pdf_path ? 'PDF ✓' : '缺 PDF'}</em>
                    </div>
                  ))}
                </div>
              )}
              {props.message && <div className="info-box">{props.message}</div>}
            </div>

            <div className="panel settings-card">
              <div className="settings-head">
                <h3>健康检查</h3>
                <button onClick={props.onCheckHealth} disabled={!currentBank}>
                  <ShieldCheck size={16} /> 检查
                </button>
              </div>
              {props.bankHealth ? (
                <>
                  <div className="health-grid">
                    <Stat label="题目" value={props.bankHealth.question_count} />
                    <Stat label="空题干" value={props.bankHealth.empty_question_count} />
                    <Stat label="缺答案" value={props.bankHealth.empty_answer_count} />
                    <Stat label="缺页码" value={props.bankHealth.missing_page_count} />
                  </div>
                  <div className="translation-health">
                    <div className="translation-health-bar">
                      <span>翻译进度</span>
                      <strong>{props.bankHealth.translated_count} / {props.bankHealth.question_count}</strong>
                    </div>
                    <progress value={props.bankHealth.translated_count} max={props.bankHealth.question_count || 1} />
                    {!props.bankHealth.translation_db_exists && (
                      <p className="muted">尚未创建翻译库，前往「翻译服务」执行批量翻译。</p>
                    )}
                    {props.bankHealth.translation_db_exists && props.bankHealth.translated_count < props.bankHealth.question_count && (
                      <p className="muted">还差 {props.bankHealth.question_count - props.bankHealth.translated_count} 题，建议前往「翻译服务」继续翻译。</p>
                    )}
                  </div>
                  <div className="health-warnings">
                    {props.bankHealth.warnings.map((w) => <p key={w}>{w}</p>)}
                  </div>
                </>
              ) : (
                <p className="muted">当前：{currentBank?.name || '无'}，点击检查查看详情。</p>
              )}
            </div>
          </div>
        </div>
      )}

      {tab === 'ai' && (
        <div className="settings-tab-body">
          <div className="panel settings-card">
            <div className="settings-head">
              <h3>AI 接口设置</h3>
              <button className="primary" onClick={() => props.onSaveAiSettings(draft)}>保存</button>
            </div>
            <label className="check-row">
              <input type="checkbox" checked={draft.enabled} onChange={(event) => setDraft({ ...draft, enabled: event.target.checked })} />
              启用 AI 分析
            </label>
            <div className="settings-form-grid">
              <label title="OpenAI 官方或兼容接口的 Base URL">
                Base URL
                <input value={draft.base_url} onChange={(event) => setDraft({ ...draft, base_url: event.target.value })} placeholder="https://api.openai.com/v1" />
              </label>
              <label title="Azure OpenAI / APIM 网关需要填写；OpenAI 官方留空">
                API Version（可选）
                <input value={draft.api_version} onChange={(event) => setDraft({ ...draft, api_version: event.target.value })} placeholder="2025-03-01-preview" />
              </label>
              <label>
                Model
                <input value={draft.model} onChange={(event) => setDraft({ ...draft, model: event.target.value })} placeholder="gpt-4.1-mini" />
              </label>
              <label>
                API Key
                <input type="password" value={draft.api_key} onChange={(event) => setDraft({ ...draft, api_key: event.target.value })} placeholder="sk-..." />
              </label>
              <label title="0.7 适合解释分析；0.2–0.4 适合答案核验">
                Temperature
                <input
                  type="number"
                  min={0}
                  max={2}
                  step={0.1}
                  value={draft.temperature}
                  onChange={(event) => setDraft({ ...draft, temperature: Number(event.target.value) })}
                />
              </label>
            </div>
            <label className="wide-field" title="预置 System 提示词，控制 AI 回答风格。例如：只分析对错原因，不要废话；不要相信预置答案，自己判断。">
              System 提示词（可选）
              <textarea
                rows={3}
                value={draft.system_prompt}
                onChange={(event) => setDraft({ ...draft, system_prompt: event.target.value })}
                placeholder="例如：回答尽量简练，只说明为什么选这个、其他为什么错。不要相信预配答案，自己独立分析。"
              />
            </label>
            <label className="wide-field" title={'点击「分析对错」按钮时使用的提示词模板。留空使用内置默认模板。'}>
              「分析对错」提示词
              <textarea
                rows={3}
                value={draft.prompt_analyze}
                onChange={(event) => setDraft({ ...draft, prompt_analyze: event.target.value })}
                placeholder="请用中文详细分析这道考试题。要求：1) 解释题干问什么；2) 解释正确答案为什么正确；3) 分析每个错误选项为什么错；4) 提炼知识点；5) 给出记忆方法。"
              />
            </label>
            <label className="wide-field" title={'点击「总结题目」按钮时使用的提示词模板。留空使用内置默认模板。'}>
              「总结题目」提示词
              <textarea
                rows={3}
                value={draft.prompt_summarize}
                onChange={(event) => setDraft({ ...draft, prompt_summarize: event.target.value })}
                placeholder="请用中文简洁总结这道考试题。要求：1) 一句话概括题目在问什么；2) 正确答案是什么；3) 核心考点是什么；4) 关键词列表。不需要逐选项分析。"
              />
            </label>
          </div>
        </div>
      )}

      {tab === 'translation' && (
        <div className="settings-tab-body">
          <div className="panel settings-card">
            <div className="settings-head">
              <h3>翻译方式</h3>
              <div className="action-row compact-actions">
                {draft.translation_provider === 'microsoft_translator' && (
                  <button onClick={() => props.onTestTranslatorSettings(draft)}>测试连接</button>
                )}
                <button className="primary" onClick={() => props.onSaveAiSettings(draft)}>保存</button>
              </div>
            </div>
            <label>
              <select
                value={draft.translation_provider}
                onChange={(event) => setDraft({ ...draft, translation_provider: event.target.value as AiSettings['translation_provider'] })}
              >
                <option value="ai">AI 翻译（复用 AI 接口）</option>
                <option value="microsoft_translator">Microsoft Translator（Azure）</option>
              </select>
            </label>
            {draft.translation_provider === 'microsoft_translator' ? (
              <div className="settings-form-grid">
                <label title="固定使用 https://api.cognitive.microsofttranslator.com/">
                  Endpoint
                  <input
                    value={draft.translator_endpoint}
                    onChange={(event) => setDraft({ ...draft, translator_endpoint: event.target.value })}
                    placeholder="https://api.cognitive.microsofttranslator.com/"
                  />
                </label>
                <label title="Azure 门户中 Translator 资源的位置/区域。区域型资源必填，全局资源可留空">
                  Region（区域型资源必填）
                  <input value={draft.translator_region} onChange={(event) => setDraft({ ...draft, translator_region: event.target.value })} placeholder="swedencentral" />
                </label>
                <label className="wide-field">
                  Translator Key
                  <input type="password" value={draft.translator_key} onChange={(event) => setDraft({ ...draft, translator_key: event.target.value })} placeholder="Azure Translator 密钥" />
                </label>
              </div>
            ) : (
              <div className="info-box">将复用 AI 接口配置进行翻译。</div>
            )}
          </div>

          <div className="panel settings-card">
            <div className="settings-head">
              <h3>全题库批量翻译</h3>
              <button className="primary" onClick={props.onBatchTranslate} disabled={!currentBank || props.batchTranslationBusy}>
                {props.batchTranslationBusy ? '翻译中...' : '开始翻译'}
              </button>
            </div>
            <p className="muted" title="翻译结果写入 .translations.sqlite，中断后可继续">
              逐题翻译当前题库，自动跳过已完成的题目。
            </p>
            {props.batchTranslation ? (
              <div className="batch-progress-box">
                <div className="batch-progress-head">
                  <strong>{batchProgressPercent}%</strong>
                  <span>{props.batchTranslation.current_index} / {props.batchTranslation.total} 题</span>
                </div>
                <progress value={props.batchTranslation.current_index} max={props.batchTranslation.total || 1} />
                <p>{props.batchTranslation.message}</p>
                <div className="batch-progress-stats">
                  <span>新翻译：{props.batchTranslation.translated}</span>
                  <span>已跳过：{props.batchTranslation.skipped}</span>
                  <span>失败：{props.batchTranslation.failed}</span>
                </div>
              </div>
            ) : (
              <div className="empty-state small">点击按钮后显示进度</div>
            )}
          </div>
        </div>
      )}
    </section>
  );
}

function ReviewList(props: {
  mode: ReviewMode;
  questions: QuestionSummary[];
  allCount: number;
  selectedId: string;
  query: string;
  sessions: ExamSessionSummary[];
  sessionId: string;
  onQuery: (value: string) => void;
  onMode: (mode: ReviewMode) => void;
  onSessionChange: (sessionId: string) => void;
  onSelect: (id: string) => void;
}) {
  return (
    <div className="panel question-list-panel">
      <div className="review-mode-grid">
        <button className={props.mode === 'wrong' ? 'selected' : ''} onClick={() => props.onMode('wrong')}>
          错题
        </button>
        <button className={props.mode === 'favorite' ? 'selected' : ''} onClick={() => props.onMode('favorite')}>
          收藏
        </button>
        <button className={props.mode === 'needs_review' ? 'selected' : ''} onClick={() => props.onMode('needs_review')}>
          待复习
        </button>
        <button className={props.mode === 'mastered' ? 'selected' : ''} onClick={() => props.onMode('mastered')}>
          已掌握
        </button>
      </div>
      {props.mode === 'wrong' && props.sessions.length > 0 && (
        <div className="review-session-filter">
          <select value={props.sessionId} onChange={(e) => props.onSessionChange(e.target.value)}>
            <option value="">全局错题</option>
            {props.sessions.map((s) => (
              <option key={s.id} value={s.id}>
                {new Date(s.finished_at).toLocaleDateString()} {new Date(s.finished_at).toLocaleTimeString([], {hour: '2-digit', minute: '2-digit'})} · {s.wrong_count} 错 / {s.total_questions} 题
              </option>
            ))}
          </select>
        </div>
      )}
      <div className="search-box">
        <Search size={16} />
        <input value={props.query} onChange={(event) => props.onQuery(event.target.value)} placeholder="搜索复习题" />
      </div>
      <div className="list-meta">
        显示 {props.questions.length} / {props.allCount} 题
      </div>
      <div className="question-grid">
        {props.questions.length === 0 ? (
          <div className="empty-state small">当前复习范围还没有题目。</div>
        ) : (
          props.questions.map((question) => (
            <button
              key={question.id}
              className={question.id === props.selectedId ? 'q-num active' : 'q-num'}
              onClick={() => props.onSelect(question.id)}
              title={question.preview}
            >
              {question.sequence_number}
            </button>
          ))
        )}
      </div>
    </div>
  );
}

function PathItem({ label, value }: { label: string; value: string }) {
  return (
    <div className="path-item">
      <span>{label}</span>
      <code>{value}</code>
    </div>
  );
}

function QuestionList(props: {
  questions: QuestionSummary[];
  allCount: number;
  selectedId: string;
  query: string;
  typeFilter: string;
  statusFilter: string;
  types: string[];
  statuses: string[];
  onQuery: (value: string) => void;
  onType: (value: string) => void;
  onStatus: (value: string) => void;
  onSelect: (id: string) => void;
}) {
  return (
    <div className="panel question-list-panel">
      <div className="filter-row">
        <div className="search-box">
          <Search size={16} />
          <input value={props.query} onChange={(event) => props.onQuery(event.target.value)} placeholder="搜索题号或题干" />
        </div>
        <select value={props.typeFilter} onChange={(event) => props.onType(event.target.value)}>
          <option value="all">全部题型</option>
          {props.types.map((type) => (
            <option key={type} value={type}>
              {type}
            </option>
          ))}
        </select>
        <select value={props.statusFilter} onChange={(event) => props.onStatus(event.target.value)}>
          <option value="all">全部状态</option>
          {props.statuses.map((status) => (
            <option key={status} value={status}>
              {status}
            </option>
          ))}
        </select>
      </div>
      <div className="list-meta">
        显示 {props.questions.length} / {props.allCount} 题
      </div>
      <div className="question-grid">
        {props.questions.map((question) => (
          <button
            key={question.id}
            className={question.id === props.selectedId ? 'q-num active' : 'q-num'}
            onClick={() => props.onSelect(question.id)}
            title={question.preview}
          >
            {question.sequence_number}
          </button>
        ))}
      </div>
    </div>
  );
}

function QuestionPanel(props: {
  detail: QuestionDetail | null;
  pages: PageImage[];
  showAnswer: boolean;
  flags?: string[];
  interaction?: InteractionModel | null;
  translations?: TranslationRow[];
  translationMode?: TranslationMode;
  translationLanguage?: string;
  aiEnabled?: boolean;
  aiPrompt?: string;
  aiAnswer?: string;
  aiBusy?: boolean;
  aiStreamChunkCount?: number;
  translationBusy?: boolean;
  pagesBusy?: boolean;
  returnLabel?: string;
  compact?: boolean;
  hideActions?: boolean;
  hideAnswerAreas?: boolean;
  selectedOptions?: string[];
  onOptionSelect?: (key: string) => void;
  onReturn?: () => void;
  onToggleFlag?: (flagType: string) => void;
  onAiPrompt?: (value: string) => void;
  onAskAi?: (actionType: 'analyze' | 'summarize' | 'freeform') => void;
  onTranslationMode?: (value: TranslationMode) => void;
  onTranslationLanguage?: (value: string) => void;
  onTranslate?: (force: boolean) => void;
  onToggleAnswer: () => void;
  onLoadPages: () => void;
  onPrev?: () => void;
  onNext?: () => void;
}) {
  const panelRef = useRef<HTMLDivElement | null>(null);
  const { detail } = props;
  if (!detail) return <div className="panel empty-state">请选择一道题。</div>;
  const expected = expectedLetters(detail.recommended_answer || '');
  const translationMap = createTranslationMap(props.translations || []);
  const translationMode = props.translationMode || 'original';
  const interactiveOptions = Boolean(props.onOptionSelect);
  const scrollPanel = (position: 'top' | 'bottom') => {
    const element = panelRef.current;
    if (!element) return;
    element.scrollTo({ top: position === 'top' ? 0 : element.scrollHeight, behavior: 'smooth' });
  };
  return (
    <div ref={panelRef} className={props.compact ? 'question-detail compact' : 'panel question-detail'}>
      <div className="detail-title">
        <div>
          <span className="eyebrow">Question {detail.sequence_number}</span>
          <h3>{detail.question_type}</h3>
        </div>
        <div className="tag-row right">
          <em>{detail.status}</em>
          <em>{detail.source_pages || 'no pages'}</em>
        </div>
      </div>

      {props.returnLabel && props.onReturn && (
        <button className="return-link" onClick={props.onReturn}>
          ← {props.returnLabel}
        </button>
      )}

      {!props.compact && props.onToggleFlag && (
        <div className="flag-row">
          <FlagButton label="收藏" flagType="favorite" active={props.flags?.includes('favorite') ?? false} onToggle={props.onToggleFlag} />
          <FlagButton label="错题" flagType="wrong" active={props.flags?.includes('wrong') ?? false} onToggle={props.onToggleFlag} />
          <FlagButton label="待复习" flagType="needs_review" active={props.flags?.includes('needs_review') ?? false} onToggle={props.onToggleFlag} />
          <FlagButton label="已掌握" flagType="mastered" active={props.flags?.includes('mastered') ?? false} onToggle={props.onToggleFlag} />
        </div>
      )}

      <section>
        <h4>题目</h4>
        <div className="text-block">{translatedNode('question_text', detail.question_text, translationMode, translationMap)}</div>
      </section>

      {!props.compact && props.translationMode && props.onTranslationMode && props.onTranslate && (
        <TranslationPanel
          rows={props.translations || []}
          mode={props.translationMode}
          language={props.translationLanguage || 'zh-CN'}
          busy={props.translationBusy ?? false}
          onMode={props.onTranslationMode}
          onLanguage={props.onTranslationLanguage}
          onTranslate={props.onTranslate}
          onPrev={props.onPrev}
          onNext={props.onNext}
        />
      )}

      {detail.options.length > 0 && (
        <section>
          <div className="option-list">
            {detail.options.map((option) => (
              <button
                className={optionClassName({
                  showAnswer: props.showAnswer,
                  isCorrect: expected.includes(option.option_key),
                  isSelected: props.selectedOptions?.includes(option.option_key) ?? false,
                  interactive: interactiveOptions,
                })}
                key={option.option_key}
                disabled={!interactiveOptions}
                onClick={() => props.onOptionSelect?.(option.option_key)}
              >
                <strong>{option.option_key}</strong>
                <span>{translatedNode(`option:${option.option_key}`, option.option_text, translationMode, translationMap)}</span>
              </button>
            ))}
          </div>
        </section>
      )}

      {!props.hideAnswerAreas && detail.answer_areas.length > 0 && (
        <section>
          <h4>答案区</h4>
          <div className="answer-table">
            {detail.answer_areas.map((row) => (
              <div className="answer-row" key={`${row.sort_order}-${row.prompt}`}>
                <span>{translatedNode(`answer_area_prompt:${row.sort_order}`, row.prompt, translationMode, translationMap)}</span>
                <strong>{translatedNode(`answer_area_recommended:${row.sort_order}`, row.recommended_selection, translationMode, translationMap)}</strong>
              </div>
            ))}
          </div>
        </section>
      )}

      {!props.compact && shouldShowInteraction(props.interaction) && <InteractionPanel model={props.interaction!} />}

      {!props.hideActions && (
        <div className="action-row">
          <button className="primary" onClick={props.onToggleAnswer}>
            {props.showAnswer ? '隐藏答案' : '显示答案'}
          </button>
          <button onClick={props.onLoadPages} disabled={props.pagesBusy}>
            <FileText size={16} /> {props.pagesBusy ? '加载中...' : '加载 PDF 对应页'}
          </button>
          {!props.compact && props.onAskAi && (
            <>
              <button disabled={!(props.aiEnabled) || (props.aiBusy ?? false)} onClick={() => props.onAskAi!('analyze')}>
                {props.aiBusy ? '生成中...' : 'AI 分析对错'}
              </button>
              <button disabled={!(props.aiEnabled) || (props.aiBusy ?? false)} onClick={() => props.onAskAi!('summarize')}>
                AI 总结
              </button>
            </>
          )}
        </div>
      )}

      {props.showAnswer && (
        <section className="answer-box">
          <div>
            <h4>源答案</h4>
            <p>{translatedNode('source_answer', detail.source_answer || '无', translationMode, translationMap)}</p>
          </div>
          <div>
            <h4>推荐答案</h4>
            <p>{translatedNode('recommended_answer', detail.recommended_answer || '无', translationMode, translationMap)}</p>
          </div>
          <div>
            <h4>我的判断</h4>
            <p>{translatedNode('chinese_judgement', detail.chinese_judgement || '无', translationMode, translationMap)}</p>
          </div>
          <div>
            <h4>Reasoning</h4>
            <p>{translatedNode('reasoning', detail.reasoning || '无', translationMode, translationMap)}</p>
          </div>
        </section>
      )}

      {!props.compact && props.onAskAi && props.onAiPrompt && (
        <AiPanel
          enabled={props.aiEnabled ?? false}
          prompt={props.aiPrompt || ''}
          answer={props.aiAnswer || ''}
          busy={props.aiBusy ?? false}
          onPrompt={props.onAiPrompt}
          onAsk={props.onAskAi}
        />
      )}

      {props.pages.length > 0 && (
        <section>
          <h4>PDF 原文页</h4>
          <div className="pdf-pages">
            {props.pages.map((page) => (
              <figure key={page.page}>
                <figcaption>Page {page.page}</figcaption>
                <img src={page.data_url} alt={`PDF page ${page.page}`} />
              </figure>
            ))}
          </div>
        </section>
      )}

      {!props.compact && (
        <div className="scroll-fab">
          <button title="到顶部" onClick={() => scrollPanel('top')}>
            <ArrowUp size={18} />
          </button>
          <button title="到底部" onClick={() => scrollPanel('bottom')}>
            <ArrowDown size={18} />
          </button>
        </div>
      )}
    </div>
  );
}

function InteractionPanel({ model }: { model: InteractionModel }) {
  return (
    <section className="interaction-panel">
      <h4>复杂题型交互框架</h4>
      <div className="info-box">{model.message}</div>
      <div className="interaction-meta">
        <em>{model.kind}</em>
        <em>{model.can_auto_grade ? '可自动判分' : '人工自评'}</em>
      </div>
      {model.rows.length > 0 && (
        <div className="answer-table">
          {model.rows.map((row) => (
            <div className="answer-row" key={row.id}>
              <span>{row.prompt}</span>
              <strong>{row.correct_selection || row.option_group || '待选择'}</strong>
            </div>
          ))}
        </div>
      )}
      {model.slots.length > 0 && (
        <div className="drag-shell">
          <div>
            <h4>候选项池</h4>
            {model.options.map((option) => <span key={option.key}>{option.text}</span>)}
          </div>
          <div>
            <h4>槽位</h4>
            {model.slots.map((slot) => (
              <div className="slot-row" key={slot.id}>{slot.label} → {slot.correct_option || '待填入'}</div>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}

function TranslationPanel(props: {
  rows: TranslationRow[];
  mode: TranslationMode;
  language: string;
  busy: boolean;
  onMode: (value: TranslationMode) => void;
  onLanguage?: (value: string) => void;
  onTranslate: (force: boolean) => void;
  onPrev?: () => void;
  onNext?: () => void;
}) {
  return (
    <section className="translation-panel">
      <div className="panel-headline">
        <div className="segmented-control">
          <button className={props.mode === 'original' ? 'active' : ''} onClick={() => props.onMode('original')}>原文</button>
          <button className={props.mode === 'translated' ? 'active' : ''} onClick={() => props.onMode('translated')}>翻译</button>
          <button className={props.mode === 'side_by_side' ? 'active' : ''} onClick={() => props.onMode('side_by_side')}>对照</button>
          {(props.onPrev || props.onNext) && (
            <>
              <button className="nav-btn" disabled={!props.onPrev} onClick={props.onPrev}>← 上一题</button>
              <button className="nav-btn" disabled={!props.onNext} onClick={props.onNext}>下一题 →</button>
            </>
          )}
        </div>
        <div className="action-row compact-actions">
          <input value={props.language} onChange={(event) => props.onLanguage?.(event.target.value)} />
          <button disabled={props.busy} onClick={() => props.onTranslate(false)}>翻译/读取缓存</button>
          <button disabled={props.busy} onClick={() => props.onTranslate(true)}>重新翻译</button>
        </div>
      </div>
    </section>
  );
}

function AiPanel(props: { enabled: boolean; prompt: string; answer: string; busy: boolean; onPrompt: (value: string) => void; onAsk: (actionType: 'analyze' | 'summarize' | 'freeform') => void }) {
  return (
    <section className="ai-panel">
      {!props.enabled && <div className="info-box">请先在控制面板启用 AI 并保存配置。</div>}
      <div className="ai-chat-shell">
        {props.answer && (
          <div className="ai-message assistant">
            <div className="ai-avatar">AI</div>
            <div className="markdown-body ai-answer">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>{props.answer}</ReactMarkdown>
              {props.busy && <span className="stream-cursor" aria-hidden="true" />}
            </div>
          </div>
        )}
      </div>
      <div className="ai-input-bar">
        <textarea value={props.prompt} onChange={(event) => props.onPrompt(event.target.value)} placeholder={'输入问题，例如：为什么 B 不对？'} />
        <button className="primary" disabled={!props.enabled || props.busy} onClick={() => props.onAsk('freeform')}>
          提问
        </button>
      </div>
    </section>
  );
}

function FlagButton(props: { label: string; flagType: string; active: boolean; onToggle: (flagType: string) => void }) {
  return (
    <button className={props.active ? 'flag-button active' : 'flag-button'} onClick={() => props.onToggle(props.flagType)}>
      <Star size={15} /> {props.label}
    </button>
  );
}

function Stat({ label, value }: { label: string; value: number }) {
  return (
    <div className="stat">
      <strong>{value}</strong>
      <span>{label}</span>
    </div>
  );
}

function viewTitle(view: View) {
  if (view === 'exam') return '考试区域';
  if (view === 'history') return '历史记录';
  if (view === 'review') return '复习模式';
  if (view === 'settings') return '控制面板';
  return '题库浏览';
}

function isFlagged(flags: QuestionFlag[], questionId: string, flagType: string) {
  return flags.some((flag) => flag.question_id === questionId && flag.flag_type === flagType);
}

function flagTypesForQuestion(flags: QuestionFlag[], questionId: string) {
  return flags.filter((flag) => flag.question_id === questionId).map((flag) => flag.flag_type);
}

function uniqueSorted(values: string[]) {
  return [...new Set(values.filter(Boolean))].sort((a, b) => a.localeCompare(b));
}

function shuffle<T>(items: T[]) {
  return items
    .map((item) => ({ item, sort: Math.random() }))
    .sort((a, b) => a.sort - b.sort)
    .map(({ item }) => item);
}

function expectedLetters(value: string) {
  const match = value.trim().toUpperCase().match(/^([A-Z](?:\s*[,;/]\s*[A-Z])*)/);
  if (!match) return [];
  return match[1]
    .split(/[,;/]/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function sameSet(left: string[], right: string[]) {
  const a = [...left].sort().join(',');
  const b = [...right].sort().join(',');
  return a === b;
}

function matchesExamCategory(questionType: string, category: ExamCategory) {
  const normalized = questionType.toLowerCase();
  if (category === 'hotspot') return normalized.includes('hotspot');
  if (category === 'single_choice') return normalized.includes('single_choice');
  return normalized.includes('multiple_choice');
}

function optionClassName(input: { showAnswer: boolean; isCorrect: boolean; isSelected: boolean; interactive: boolean }) {
  return [
    'option-item',
    input.showAnswer && input.isCorrect ? 'correct-answer-option' : '',
    input.isSelected ? 'selected-answer-option' : '',
    input.interactive ? 'interactive-option' : '',
  ]
    .filter(Boolean)
    .join(' ');
}

function shouldShowInteraction(model?: InteractionModel | null) {
  if (!model) return false;
  if (model.kind === 'manual' || model.kind === 'single_choice' || model.kind === 'multiple_choice') return false;
  return model.rows.length > 0 || model.slots.length > 0;
}

function createTranslationMap(rows: TranslationRow[]) {
  const map = new Map<string, string>();
  const grouped = new Map<string, TranslationRow[]>();
  for (const row of rows) {
    map.set(`${row.field_name}:${row.segment_index}`, row.translated_text);
    grouped.set(row.field_name, [...(grouped.get(row.field_name) || []), row]);
  }
  for (const [fieldName, fieldRows] of grouped) {
    if (fieldRows.length > 1) {
      const merged = [...fieldRows]
        .sort((left, right) => left.segment_index - right.segment_index)
        .map((row) => row.translated_text)
        .join('\n\n');
      map.set(`${fieldName}:0`, merged);
    }
  }
  return map;
}

function translatedNode(fieldName: string, original: string, mode: TranslationMode, translations: Map<string, string>) {
  const translated = translations.get(`${fieldName}:0`);
  if (mode === 'translated' && translated) return translated;
  if (mode === 'side_by_side' && translated) {
    return (
      <span className="translation-inline">
        <span>{original}</span>
        <span>{translated}</span>
      </span>
    );
  }
  return original;
}

function formatDuration(seconds: number) {
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}m ${secs}s`;
}

export default App;
