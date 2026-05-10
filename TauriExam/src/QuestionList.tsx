import { Search } from 'lucide-react';
import type { ExamSessionSummary, QuestionPracticeStats, QuestionSummary, ReviewMode } from './types';

export function Stat({ label, value }: { label: string; value: number }) {
  return (
    <div className="stat">
      <strong>{value}</strong>
      <span>{label}</span>
    </div>
  );
}

export function ReviewList(props: {
  mode: ReviewMode;
  questions: QuestionSummary[];
  allCount: number;
  selectedId: string;
  query: string;
  sessions: ExamSessionSummary[];
  sessionId: string;
  topics: string[];
  topicFilter: string;
  practiceStats: Record<string, QuestionPracticeStats>;
  onQuery: (value: string) => void;
  onTopic: (value: string) => void;
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
      <select value={props.topicFilter} onChange={(event) => props.onTopic(event.target.value)}>
        <option value="all">全部知识点</option>
        {props.topics.map((topic) => (
          <option key={topic} value={topic}>
            {topic}
          </option>
        ))}
      </select>
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
              <span>{question.sequence_number}</span>
              {props.practiceStats[question.id]?.wrong_count > 0 && <em>错 {props.practiceStats[question.id].wrong_count}</em>}
            </button>
          ))
        )}
      </div>
    </div>
  );
}

export default function QuestionList(props: {
  questions: QuestionSummary[];
  allCount: number;
  selectedId: string;
  query: string;
  typeFilter: string;
  statusFilter: string;
  topicFilter: string;
  types: string[];
  statuses: string[];
  topics: string[];
  practiceStats: Record<string, QuestionPracticeStats>;
  onQuery: (value: string) => void;
  onType: (value: string) => void;
  onStatus: (value: string) => void;
  onTopic: (value: string) => void;
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
        <select value={props.topicFilter} onChange={(event) => props.onTopic(event.target.value)}>
          <option value="all">全部知识点</option>
          {props.topics.map((topic) => (
            <option key={topic} value={topic}>
              {topic}
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
              <span>{question.sequence_number}</span>
              {props.practiceStats[question.id]?.wrong_count > 0 && <em>错 {props.practiceStats[question.id].wrong_count}</em>}
          </button>
        ))}
      </div>
    </div>
  );
}
