// 题目 → 设备槽位映射。纯函数，无副作用，便于单测与后续整改。
//
// 设备布局（AKP153，5 列 × 3 行 + 3 块右侧屏）：
//   key-1  key-2  key-3  key-4  key-5     display-1 = 进度
//   key-6  key-7  (空)   (空)   (空)      display-2 = 题型
//   key-11 key-12 (空)   (空)   key-15    display-3 = (预留)
//   = ◀上一题  ▶下一题            ✔提交
//
// 按键驱动走 /events 轮询（见 interpretSlot），不再用 emit_key 注入键盘：
// 既能在无真实硬件（虚拟设备）下联调，也避免 TauriExam 不在前台时把按键打进别的程序。

import type { DeckSlotInput, QuestionDetail } from '../types';
import { arrowIcon, letterIcon, submitIcon } from './deckIcons';

/** 选项字母 → 槽位 id。最多 7 个，对应现有 OPTION_HOTKEYS(A-G)。 */
const OPTION_SLOT_IDS = ['key-1', 'key-2', 'key-3', 'key-4', 'key-5', 'key-6', 'key-7'] as const;

const SLOT_PREV = 'key-11';
const SLOT_NEXT = 'key-12';
const SLOT_SUBMIT = 'key-15';

const DISPLAY_PROGRESS = 'display-1';
const DISPLAY_TYPE = 'display-2';

/** 颜色常量，与 UI 语义保持一致。 */
export const DECK_COLORS = {
  option: '#222a3a',
  optionSelected: '#2ecc71',
  nav: '#1e6fff',
  submit: '#e0a000',
  display: '#000000',
} as const;

export type DeckExamContext = {
  detail: QuestionDetail;
  /** 当前已选选项字母（多选可能多个）。 */
  selected: string[];
  index: number;
  total: number;
  /** 是否启用翻题键（仅在宿主界面已接线 ←/→ 时为 true；考试视图为 false）。 */
  navEnabled: boolean;
  hasPrev: boolean;
  hasNext: boolean;
};

function isChoiceQuestion(detail: QuestionDetail): boolean {
  const type = (detail.question_type || '').toLowerCase();
  return (
    (type.includes('single_choice') || type.includes('multiple_choice')) &&
    detail.options.length > 0
  );
}

/** 当前题是否能投到设备（阶段 1 仅单选 / 多选）。 */
export function isDeckSupported(detail: QuestionDetail | null): boolean {
  return Boolean(detail && isChoiceQuestion(detail));
}

/**
 * 把当前题映射为整页槽位。配合 clear_first=true 使用，保证旧内容被清掉。
 * selectedSet 命中的选项会高亮成绿色。
 */
export function buildSlots(ctx: DeckExamContext): DeckSlotInput[] {
  const slots: DeckSlotInput[] = [];
  const selectedSet = new Set(ctx.selected.map((k) => k.toUpperCase()));

  // 选项 A..G
  ctx.detail.options.slice(0, OPTION_SLOT_IDS.length).forEach((opt, i) => {
    const letter = opt.option_key.toUpperCase();
    const selected = selectedSet.has(letter);
    slots.push({
      slot_id: OPTION_SLOT_IDS[i],
      icon: letterIcon(letter, selected),
      title: letter,
      color: selected ? DECK_COLORS.optionSelected : DECK_COLORS.option,
    });
  });

  // 翻题（仅宿主界面已接线时显示，避免死键）
  if (ctx.navEnabled) {
    slots.push({
      slot_id: SLOT_PREV,
      icon: arrowIcon('left', ctx.hasPrev),
      title: '上一题',
      color: DECK_COLORS.nav,
    });
    slots.push({
      slot_id: SLOT_NEXT,
      icon: arrowIcon('right', ctx.hasNext),
      title: '下一题',
      color: DECK_COLORS.nav,
    });
  }

  // 提交
  slots.push({
    slot_id: SLOT_SUBMIT,
    icon: submitIcon(),
    title: '提交',
    color: DECK_COLORS.submit,
  });

  // 右侧屏
  slots.push({
    slot_id: DISPLAY_PROGRESS,
    title: `第 ${ctx.index + 1}/${ctx.total} 题`,
    color: DECK_COLORS.display,
  });
  slots.push({
    slot_id: DISPLAY_TYPE,
    title: (ctx.detail.question_type || '').toLowerCase().includes('multiple') ? '多选' : '单选',
    color: DECK_COLORS.display,
  });

  return slots;
}

/** slot_id → 选项字母（用于解读 /events）。非选项槽位返回 null。 */
export function slotToOptionLetter(slotId: string, detail: QuestionDetail): string | null {
  const idx = OPTION_SLOT_IDS.indexOf(normalizeSlotId(slotId) as (typeof OPTION_SLOT_IDS)[number]);
  if (idx < 0 || idx >= detail.options.length) return null;
  return detail.options[idx].option_key.toUpperCase();
}

/**
 * 设备上报的 slot_id 可能是补零格式（如 `key-07`、`display-01`），
 * 而我们的槽位常量是不补零的 `key-7`。这里去掉数字前导零，保证匹配。
 */
function normalizeSlotId(slotId: string): string {
  const key = /^key-(\d+)$/.exec(slotId);
  if (key) return `key-${parseInt(key[1], 10)}`;
  const disp = /^display-(\d+)$/.exec(slotId);
  if (disp) return `display-${parseInt(disp[1], 10)}`;
  return slotId;
}

/** 设备按键事件 → 答题动作。供 useExamDeck 解读 /events。 */
export type DeckAction =
  | { kind: 'option'; letter: string }
  | { kind: 'submit' }
  | { kind: 'prev' }
  | { kind: 'next' };

/**
 * 把一次按键（slot_id）翻译为答题动作。
 * navEnabled=false 时忽略翻题键，避免在未接线翻题的视图里产生死动作。
 */
export function interpretSlot(
  slotId: string,
  detail: QuestionDetail,
  navEnabled: boolean,
): DeckAction | null {
  const id = normalizeSlotId(slotId);
  const letter = slotToOptionLetter(id, detail);
  if (letter) return { kind: 'option', letter };
  if (id === SLOT_SUBMIT) return { kind: 'submit' };
  if (navEnabled && id === SLOT_PREV) return { kind: 'prev' };
  if (navEnabled && id === SLOT_NEXT) return { kind: 'next' };
  return null;
}
