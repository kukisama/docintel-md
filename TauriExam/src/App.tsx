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
  const [appPaths, setAppPaths] = useState<AppPaths | null>(null);
  const [bankHealth, setBankHealth] = useState<BankHealth | null>(null);
  const [settingsMessage, setSettingsMessage] = useState('');
  const [flags, setFlags] = useState<QuestionFlag[]>([]);
  const [reviewMode, setReviewMode] = useState<ReviewMode>('wrong');
  const [reviewQuestions, setReviewQuestions] = useState<QuestionSummary[]>([]);
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

  async function loadReview(nextMode = reviewMode) {
    try {
      setLoading('正在读取复习题...');
      setError('');
      const loaded = await api.listReviewQuestions(bankId, nextMode);
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

  async function askAi() {
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
                          historyAnswers[item.id].map((answer, index) => (
                            <button className="history-answer-row" key={answer.id} onClick={() => openHistoryQuestion(answer)}>
                              <span>第 {index + 1} 题 / 原题 Q{answer.sequence_number}</span>
                              <span>{answer.user_answer || '未作答'}</span>
                              <span>{formatDuration(answer.duration_seconds)}</span>
                              <strong className={answer.is_correct ? 'ok' : answer.is_correct === false ? 'bad' : ''}>
                                {answer.is_correct ? '正确' : answer.is_correct === false ? '错误' : '复核'}
                              </strong>
                            </button>
                          ))
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
              onQuery={setQuery}
              onMode={(mode) => {
                setReviewMode(mode);
                loadReview(mode);
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
            onOpenDataDir={openDataDir}
            onOpenQuestionBanksDir={openQuestionBanksDir}
            onRefreshBanks={refreshQuestionBanks}
            onCheckHealth={checkCurrentBankHealth}
            onSaveAiSettings={saveAiSettings}
            onTestTranslatorSettings={testTranslatorSettings}
          />
        )}
      </main>
    </div>
  );
}

function SettingsPanel(props: {
  appPaths: AppPaths | null;
  banks: BankInfo[];
  bankId: string;
  bankHealth: BankHealth | null;
  aiSettings: AiSettings | null;
  message: string;
  onOpenDataDir: () => void;
  onOpenQuestionBanksDir: () => void;
  onRefreshBanks: () => void;
  onCheckHealth: () => void;
  onSaveAiSettings: (settings: AiSettings) => void;
  onTestTranslatorSettings: (settings: AiSettings) => void;
}) {
  const currentBank = props.banks.find((bank) => bank.id === props.bankId);
  const [draft, setDraft] = useState<AiSettings>(
    props.aiSettings || {
      enabled: false,
      base_url: 'https://api.openai.com/v1',
      api_version: '',
      api_key: '',
      model: 'gpt-4.1-mini',
      temperature: 0.7,
      translation_provider: 'ai',
      translator_endpoint: 'https://api.cognitive.microsofttranslator.com',
      translator_key: '',
      translator_region: '',
    },
  );

  useEffect(() => {
    if (props.aiSettings) setDraft(props.aiSettings);
  }, [props.aiSettings]);

  return (
    <section className="settings-grid">
      <div className="panel settings-card wide">
        <div className="settings-head">
          <div>
            <span className="eyebrow">Local workspace</span>
            <h3>数据目录与题库目录</h3>
          </div>
          <div className="action-row compact-actions">
            <button onClick={props.onOpenDataDir}>
              <FolderOpen size={16} /> 打开数据目录
            </button>
            <button onClick={props.onOpenQuestionBanksDir}>
              <FolderOpen size={16} /> 打开题库目录
            </button>
          </div>
        </div>
        <div className="path-list">
          <PathItem label="应用数据目录" value={props.appPaths?.data_dir || '加载中...'} />
          <PathItem label="学习数据库" value={props.appPaths?.app_db_path || '加载中...'} />
          <PathItem label="题库目录" value={props.appPaths?.question_banks_dir || '加载中...'} />
          <PathItem label="PDF 缓存目录" value={props.appPaths?.page_cache_dir || '加载中...'} />
        </div>
        <p className="muted">把同名文件放入题库目录即可识别：例如 `SC-100.sqlite` + `SC-100.pdf`。</p>
      </div>

      <div className="panel settings-card">
        <div className="settings-head">
          <div>
            <span className="eyebrow">Question banks</span>
            <h3>题库刷新</h3>
          </div>
          <button className="primary" onClick={props.onRefreshBanks}>
            <RefreshCw size={16} /> 刷新
          </button>
        </div>
        <div className="bank-mini-list">
          {props.banks.length === 0 ? (
            <div className="empty-state small">还没有识别到题库。</div>
          ) : (
            props.banks.map((bank) => (
              <div className="bank-mini-row" key={bank.id}>
                <strong>{bank.name}</strong>
                <span>{bank.question_count} 题</span>
                <em>{bank.pdf_path ? 'PDF 已匹配' : '缺少 PDF'}</em>
              </div>
            ))
          )}
        </div>
        {props.message && <div className="info-box">{props.message}</div>}
      </div>

      <div className="panel settings-card">
        <div className="settings-head">
          <div>
            <span className="eyebrow">Health check</span>
            <h3>当前题库健康检查</h3>
          </div>
          <button onClick={props.onCheckHealth} disabled={!currentBank}>
            <ShieldCheck size={16} /> 检查
          </button>
        </div>
        <p className="muted">当前题库：{currentBank?.name || '无'}</p>
        {props.bankHealth ? (
          <div className="health-grid">
            <Stat label="题目" value={props.bankHealth.question_count} />
            <Stat label="空题干" value={props.bankHealth.empty_question_count} />
            <Stat label="缺答案" value={props.bankHealth.empty_answer_count} />
            <Stat label="缺页码" value={props.bankHealth.missing_page_count} />
            <div className="health-warnings">
              {props.bankHealth.warnings.map((warning) => (
                <p key={warning}>{warning}</p>
              ))}
            </div>
          </div>
        ) : (
          <div className="empty-state small">点击检查后显示题库状态。</div>
        )}
      </div>

      <div className="panel settings-card wide muted-card">
        <div className="settings-head">
          <div>
            <span className="eyebrow">OpenAI-compatible Responses</span>
            <h3>AI 接口设置</h3>
          </div>
          <button className="primary" onClick={() => props.onSaveAiSettings(draft)}>保存 AI 设置</button>
        </div>
        <label className="check-row">
          <input type="checkbox" checked={draft.enabled} onChange={(event) => setDraft({ ...draft, enabled: event.target.checked })} />
          启用 AI 分析与 OpenAI 翻译
        </label>
        <div className="settings-form-grid">
          <label>
            Base URL
            <input value={draft.base_url} onChange={(event) => setDraft({ ...draft, base_url: event.target.value })} placeholder="https://api.openai.com/v1" />
          </label>
          <label>
            API Version（Azure/APIM 可选）
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
          <label>
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
        <p>
          启用后，当前题目/选项/答案/解析会发送到你配置的 AI 服务。OpenAI 官方接口通常不填 API Version；Azure/APIM 网关通常需要填写。
          Temperature 默认 0.7，更适合解释型分析；如需更保守稳定的答案核验，可降到 0.2–0.4。
        </p>
      </div>

      <div className="panel settings-card wide muted-card">
        <div className="settings-head">
          <div>
            <span className="eyebrow">Translation provider</span>
            <h3>题目翻译服务</h3>
          </div>
          <div className="action-row compact-actions">
            <button onClick={() => props.onTestTranslatorSettings(draft)}>测试 Microsoft Translator</button>
            <button className="primary" onClick={() => props.onSaveAiSettings(draft)}>保存翻译设置</button>
          </div>
        </div>
        <label>
          翻译方式
          <select
            value={draft.translation_provider}
            onChange={(event) => setDraft({ ...draft, translation_provider: event.target.value as AiSettings['translation_provider'] })}
          >
            <option value="ai">AI 翻译（走上方 OpenAI-compatible Responses）</option>
            <option value="microsoft_translator">Microsoft Translator（Azure AI Translator）</option>
          </select>
        </label>
        {draft.translation_provider === 'microsoft_translator' ? (
          <>
            <div className="settings-form-grid">
              <label>
                Translator Endpoint
                <input
                  value={draft.translator_endpoint}
                  onChange={(event) => setDraft({ ...draft, translator_endpoint: event.target.value })}
                  placeholder="https://api.cognitive.microsofttranslator.com 或 https://southeastasia.api.cognitive.microsoft.com"
                />
              </label>
              <label>
                Region
                <input value={draft.translator_region} onChange={(event) => setDraft({ ...draft, translator_region: event.target.value })} placeholder="eastasia / eastus / ..." />
              </label>
              <label className="wide-field">
                Translator Key
                <input type="password" value={draft.translator_key} onChange={(event) => setDraft({ ...draft, translator_key: event.target.value })} placeholder="Azure Translator key" />
              </label>
            </div>
            <p>
              选择 Microsoft Translator 后，题目翻译不再依赖 AI 开关，也不会调用大模型；它直接使用 Azure Translator Text API，速度更快，适合批量翻译题库。当前仅支持形如 https://southeastasia.api.cognitive.microsoft.com 的区域 Cognitive endpoint；Region 填 southeastasia。测试按钮会直接使用当前表单内容，不需要先保存。
            </p>
          </>
        ) : (
          <div className="info-box">当前选择 AI 翻译，将复用上方 OpenAI-compatible Responses 配置；无需填写 Microsoft Translator Endpoint、Region 或 Key。</div>
        )}
      </div>
    </section>
  );
}

function ReviewList(props: {
  mode: ReviewMode;
  questions: QuestionSummary[];
  allCount: number;
  selectedId: string;
  query: string;
  onQuery: (value: string) => void;
  onMode: (mode: ReviewMode) => void;
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
      <div className="search-box">
        <Search size={16} />
        <input value={props.query} onChange={(event) => props.onQuery(event.target.value)} placeholder="搜索复习题" />
      </div>
      <div className="list-meta">
        显示 {props.questions.length} / {props.allCount} 题
      </div>
      <div className="question-list">
        {props.questions.length === 0 ? (
          <div className="empty-state small">当前复习范围还没有题目。</div>
        ) : (
          props.questions.map((question) => (
            <button
              key={question.id}
              className={question.id === props.selectedId ? 'question-card active' : 'question-card'}
              onClick={() => props.onSelect(question.id)}
            >
              <div className="question-card-head">
                <strong>Q{question.sequence_number}</strong>
                <span>{question.question_type}</span>
              </div>
              <p>{question.preview}</p>
              <div className="tag-row">
                <em>{question.status}</em>
                <em>
                  p.{question.page_from ?? '?'}–{question.page_to ?? '?'}
                </em>
              </div>
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
      <div className="question-list">
        {props.questions.map((question) => (
          <button
            key={question.id}
            className={question.id === props.selectedId ? 'question-card active' : 'question-card'}
            onClick={() => props.onSelect(question.id)}
          >
            <div className="question-card-head">
              <strong>Q{question.sequence_number}</strong>
              <span>{question.question_type}</span>
            </div>
            <p>{question.preview}</p>
            <div className="tag-row">
              <em>{question.status}</em>
              <em>
                p.{question.page_from ?? '?'}–{question.page_to ?? '?'}
              </em>
            </div>
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
  onAskAi?: () => void;
  onTranslationMode?: (value: TranslationMode) => void;
  onTranslationLanguage?: (value: string) => void;
  onTranslate?: (force: boolean) => void;
  onToggleAnswer: () => void;
  onLoadPages: () => void;
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
          chunkCount={props.aiStreamChunkCount ?? 0}
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
}) {
  return (
    <section className="translation-panel">
      <div className="panel-headline">
        <div className="segmented-control">
          <button className={props.mode === 'original' ? 'active' : ''} onClick={() => props.onMode('original')}>原文</button>
          <button className={props.mode === 'translated' ? 'active' : ''} onClick={() => props.onMode('translated')}>翻译</button>
          <button className={props.mode === 'side_by_side' ? 'active' : ''} onClick={() => props.onMode('side_by_side')}>对照</button>
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

function AiPanel(props: { enabled: boolean; prompt: string; answer: string; busy: boolean; chunkCount: number; onPrompt: (value: string) => void; onAsk: () => void }) {
  return (
    <section className="ai-panel">
      <div className="panel-headline">
        <div>
          <h4>AI 题目助手</h4>
          <p className="muted">Channel 真流式 · Markdown 渲染 · {props.chunkCount} chunks</p>
        </div>
        {props.busy && <span className="stream-pill">正在生成</span>}
      </div>
      {!props.enabled && <div className="info-box">请先在控制面板启用 AI 并保存 OpenAI 兼容接口配置。</div>}
      <div className="ai-chat-shell">
        {props.answer ? (
          <div className="ai-message assistant">
            <div className="ai-avatar">AI</div>
            <div className="markdown-body ai-answer">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>{props.answer}</ReactMarkdown>
              {props.busy && <span className="stream-cursor" aria-hidden="true" />}
            </div>
          </div>
        ) : (
          <div className="ai-empty-state">输入追问后点击分析，回答会在这里按 Markdown 流式出现。</div>
        )}
      </div>
      <div className="ai-input-bar">
        <textarea value={props.prompt} onChange={(event) => props.onPrompt(event.target.value)} placeholder="可输入追问，例如：为什么 B 不对？" />
        <button className="primary" disabled={!props.enabled || props.busy} onClick={props.onAsk}>
          {props.busy ? '生成中...' : '分析当前题'}
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
