import { useEffect, useMemo, useRef, useState } from 'react';
import {
  ArrowDown,
  ArrowUp,
  BookOpen,
  Clock3,
  FileText,
  History,
  Moon,
  PlayCircle,
  Search,
  Sun,
} from 'lucide-react';
import { api } from './api';
import type {
  BankInfo,
  ExamAnswerDetail,
  ExamAnswerInput,
  ExamSessionSummary,
  PageImage,
  QuestionDetail,
  QuestionSummary,
} from './types';

type View = 'browse' | 'exam' | 'history';
type Theme = 'light' | 'dark';
type ExamCategory = 'single_choice' | 'multiple_choice' | 'hotspot';

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

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem('tauri-exam-theme', theme);
  }, [theme]);

  useEffect(() => {
    bootstrap();
  }, []);

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

  async function bootstrap() {
    try {
      setError('');
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
      if (loaded[0]) setSelectedId((current) => current || loaded[0].id);
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
      setPages([]);
      setShowAnswer(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading('');
    }
  }

  async function loadPages() {
    if (!detail) return;
    try {
      setLoading('正在加载 PDF 页...');
      setError('');
      setPages(await api.getSourcePages(bankId, detail.id));
    } catch (err) {
      setError(String(err));
    } finally {
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
    setView('browse');
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
      </main>
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
  compact?: boolean;
  hideActions?: boolean;
  hideAnswerAreas?: boolean;
  selectedOptions?: string[];
  onOptionSelect?: (key: string) => void;
  onToggleAnswer: () => void;
  onLoadPages: () => void;
}) {
  const panelRef = useRef<HTMLDivElement | null>(null);
  const { detail } = props;
  if (!detail) return <div className="panel empty-state">请选择一道题。</div>;
  const expected = expectedLetters(detail.recommended_answer || '');
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

      <section>
        <h4>题目</h4>
        <div className="text-block">{detail.question_text}</div>
      </section>

      {detail.options.length > 0 && (
        <section>
          <h4>备选答案</h4>
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
                <span>{option.option_text}</span>
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
                <span>{row.prompt}</span>
                <strong>{row.recommended_selection}</strong>
              </div>
            ))}
          </div>
        </section>
      )}

      {!props.hideActions && (
        <div className="action-row">
          <button className="primary" onClick={props.onToggleAnswer}>
            {props.showAnswer ? '隐藏答案' : '显示答案'}
          </button>
          <button onClick={props.onLoadPages}>
            <FileText size={16} /> 加载 PDF 对应页
          </button>
        </div>
      )}

      {props.showAnswer && (
        <section className="answer-box">
          <div>
            <h4>源答案</h4>
            <p>{detail.source_answer || '无'}</p>
          </div>
          <div>
            <h4>推荐答案</h4>
            <p>{detail.recommended_answer || '无'}</p>
          </div>
          <div>
            <h4>我的判断</h4>
            <p>{detail.chinese_judgement || '无'}</p>
          </div>
          <div>
            <h4>Reasoning</h4>
            <p>{detail.reasoning || '无'}</p>
          </div>
        </section>
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
  return '题库浏览';
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

function formatDuration(seconds: number) {
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}m ${secs}s`;
}

export default App;
