import type { QuestionFlag, TranslationRow } from './types';

export type View = 'browse' | 'exam' | 'history' | 'review' | 'settings';
export type Theme = 'light' | 'dark';
export type ExamCategory = 'single_choice' | 'multiple_choice' | 'hotspot' | 'drag_drop';
export type TranslationMode = 'original' | 'translated' | 'side_by_side';

export type ReturnContext = {
  view: 'history';
  sessionId: string;
  label: string;
};

export type ExamDraft = {
  startedAt: number;
  questions: import('./types').QuestionSummary[];
  index: number;
  answers: import('./types').ExamAnswerInput[];
  selected: string[];
  questionStartedAt: number;
  finished: boolean;
};

export function viewTitle(view: View) {
  if (view === 'exam') return '考试区域';
  if (view === 'history') return '历史记录';
  if (view === 'review') return '复习模式';
  if (view === 'settings') return '控制面板';
  return '题库浏览';
}

export function isFlagged(flags: QuestionFlag[], questionId: string, flagType: string) {
  return flags.some((flag) => flag.question_id === questionId && flag.flag_type === flagType);
}

export function flagTypesForQuestion(flags: QuestionFlag[], questionId: string) {
  return flags.filter((flag) => flag.question_id === questionId).map((flag) => flag.flag_type);
}

export function uniqueSorted(values: string[]) {
  return [...new Set(values.filter(Boolean))].sort((a, b) => a.localeCompare(b));
}

export function shuffle<T>(items: T[]) {
  return items
    .map((item) => ({ item, sort: Math.random() }))
    .sort((a, b) => a.sort - b.sort)
    .map(({ item }) => item);
}

export function expectedLetters(value: string) {
  const match = value.trim().toUpperCase().match(/^([A-Z](?:\s*[,;/]\s*[A-Z])*)/);
  if (!match) return [];
  return match[1]
    .split(/[,;/]/)
    .map((item) => item.trim())
    .filter(Boolean);
}

export function sameSet(left: string[], right: string[]) {
  const a = [...left].sort().join(',');
  const b = [...right].sort().join(',');
  return a === b;
}

export function matchesExamCategory(questionType: string, category: ExamCategory) {
  const normalized = questionType.toLowerCase();
  if (category === 'hotspot') return normalized.includes('hotspot');
  if (category === 'drag_drop') return normalized.includes('drag') || normalized.includes('drop');
  if (category === 'single_choice') return normalized.includes('single_choice');
  return normalized.includes('multiple_choice');
}

export function optionClassName(input: { showAnswer: boolean; isCorrect: boolean; isSelected: boolean; interactive: boolean }) {
  return [
    'option-item',
    input.showAnswer && input.isCorrect ? 'correct-answer-option' : '',
    input.isSelected ? 'selected-answer-option' : '',
    input.interactive ? 'interactive-option' : '',
  ]
    .filter(Boolean)
    .join(' ');
}

export function shouldShowInteraction(model?: import('./types').InteractionModel | null) {
  if (!model) return false;
  if (model.kind === 'manual' || model.kind === 'single_choice' || model.kind === 'multiple_choice') return false;
  return model.rows.length > 0 || model.slots.length > 0;
}

export function createTranslationMap(rows: TranslationRow[]) {
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

export function translatedNode(fieldName: string, original: string, mode: TranslationMode, translations: Map<string, string>) {
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

export function formatDuration(seconds: number) {
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}m ${secs}s`;
}
