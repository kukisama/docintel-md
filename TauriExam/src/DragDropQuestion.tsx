import { useMemo } from 'react';
import type { DragDropUserAnswer, InteractionModel, InteractionOption } from './types';
import type { TranslationMode } from './helpers';
import { translatedNode } from './helpers';

export function createEmptyDragAnswer(model: InteractionModel): DragDropUserAnswer {
  return {
    kind: 'drag_drop',
    slots: model.slots.map((slot) => ({ slot_id: slot.id, option_id: null })),
  };
}

export function createCorrectDragAnswer(model: InteractionModel): DragDropUserAnswer {
  return {
    kind: 'drag_drop',
    slots: model.slots.map((slot) => ({ slot_id: slot.id, option_id: slot.correct_option ?? null })),
  };
}

export function normalizeDragAnswer(model: InteractionModel, value?: DragDropUserAnswer | null): DragDropUserAnswer {
  const existing = new Map((value?.slots || []).map((slot) => [slot.slot_id, slot.option_id]));
  return {
    kind: 'drag_drop',
    slots: model.slots.map((slot) => ({ slot_id: slot.id, option_id: existing.get(slot.id) ?? null })),
  };
}

export function parseDragAnswer(value?: string | null): DragDropUserAnswer | null {
  if (!value) return null;
  try {
    const parsed = JSON.parse(value) as Partial<DragDropUserAnswer>;
    if (parsed?.kind !== 'drag_drop' || !Array.isArray(parsed.slots)) return null;
    return {
      kind: 'drag_drop',
      slots: parsed.slots
        .filter((slot) => typeof slot?.slot_id === 'string')
        .map((slot) => ({ slot_id: slot.slot_id, option_id: typeof slot.option_id === 'string' ? slot.option_id : null })),
    };
  } catch {
    return null;
  }
}

export function isDragDropComplete(model: InteractionModel, answer?: DragDropUserAnswer | null) {
  const normalized = normalizeDragAnswer(model, answer);
  return model.slots.length > 0 && normalized.slots.every((slot) => Boolean(slot.option_id));
}

export function gradeDragDrop(model: InteractionModel, answer: DragDropUserAnswer): boolean {
  const slotMap = new Map(answer.slots.map((slot) => [slot.slot_id, slot.option_id]));
  return model.slots.length > 0 && model.slots.every((slot) => slotMap.get(slot.id) === slot.correct_option);
}

export function describeDragAnswer(model: InteractionModel, answer: DragDropUserAnswer) {
  const slotMap = new Map(answer.slots.map((slot) => [slot.slot_id, slot.option_id]));
  return model.slots.map((slot) => `${slot.id}=${slotMap.get(slot.id) || '未填'}`).join('; ');
}

export default function DragDropQuestion(props: {
  model: InteractionModel;
  value?: DragDropUserAnswer | null;
  showAnswer?: boolean;
  showInternalIds?: boolean;
  translationMode?: TranslationMode;
  translationMap?: Map<string, string>;
  disabled?: boolean;
  onChange?: (value: DragDropUserAnswer) => void;
}) {
  const answer = useMemo(() => normalizeDragAnswer(props.model, props.value), [props.model, props.value]);
  const optionById = useMemo(() => new Map(props.model.options.map((option) => [option.key, option])), [props.model.options]);
  const usedOptionIds = new Set(answer.slots.map((slot) => slot.option_id).filter(Boolean) as string[]);
  const readonly = props.disabled || !props.onChange;
  const translationMode = props.translationMode || 'original';
  const translationMap = props.translationMap || new Map<string, string>();

  function assignOptionToSlot(slotId: string, optionId: string | null) {
    if (readonly) return;
    const nextSlots = answer.slots.map((slot) => {
      if (optionId && slot.option_id === optionId) {
        return { ...slot, option_id: null };
      }
      if (slot.slot_id === slotId) {
        return { ...slot, option_id: optionId };
      }
      return slot;
    });
    props.onChange?.({ kind: 'drag_drop', slots: nextSlots });
  }

  function optionClass(option: InteractionOption) {
    return [
      'drag-option-card',
      usedOptionIds.has(option.key) ? 'used' : '',
      option.is_distractor ? 'distractor' : '',
    ]
      .filter(Boolean)
      .join(' ');
  }

  function slotClass(slotId: string, optionId: string | null, correctOption: string | null) {
    const isCorrect = Boolean(optionId && correctOption && optionId === correctOption);
    const isWrong = Boolean(props.showAnswer && optionId && correctOption && optionId !== correctOption);
    return ['drag-slot-card', optionId ? 'filled' : '', props.showAnswer && isCorrect ? 'correct' : '', isWrong ? 'wrong' : '']
      .filter(Boolean)
      .join(' ');
  }

  function optionLabel(option: InteractionOption | undefined, optionId: string | null) {
    if (!optionId) return '未填入';
    return option ? translatedNode(`interaction_option:${option.key}`, option.text, translationMode, translationMap) : optionId;
  }

  return (
    <section className="drag-drop-board">
      <div className="drag-drop-head">
        <div>
          <h4>拖拽作答</h4>
          <p>{readonly ? '只读模式：查看槽位、你的作答和正确答案。' : '把左侧候选项拖到右侧槽位；需要调整时可拖入其它槽位或清空。'}</p>
        </div>
        <span>{isDragDropComplete(props.model, answer) ? '已填完' : '未填完'}</span>
      </div>

      <div className="drag-drop-grid">
        <div className="drag-option-pool">
          <h4>候选项池</h4>
          {props.model.options.map((option, index) => (
            <div
              key={option.key}
              role="button"
              aria-disabled={readonly}
              tabIndex={readonly ? -1 : 0}
              className={optionClass(option)}
              draggable={!readonly}
              onDragStart={(event) => {
                event.dataTransfer.effectAllowed = 'move';
                event.dataTransfer.setData('text/plain', option.key);
              }}
              title={props.showInternalIds ? undefined : `内部ID：${option.key}`}
            >
              <strong>{props.showInternalIds ? option.key : String.fromCharCode(65 + index)}</strong>
              <span>{translatedNode(`interaction_option:${option.key}`, option.text, translationMode, translationMap)}</span>
              {props.showAnswer && option.is_distractor && <em>干扰项</em>}
            </div>
          ))}
        </div>

        <div className="drag-slot-list">
          <h4>槽位</h4>
          {props.model.slots.map((slot) => {
            const current = answer.slots.find((item) => item.slot_id === slot.id)?.option_id ?? null;
            const option = optionById.get(current || '');
            const correct = slot.correct_option ? optionById.get(slot.correct_option) : undefined;
            return (
              <div
                key={slot.id}
                className={slotClass(slot.id, current, slot.correct_option)}
                onDragOver={(event) => {
                  if (!readonly) event.preventDefault();
                }}
                onDragEnter={(event) => {
                  if (!readonly) event.preventDefault();
                }}
                onDrop={(event) => {
                  if (readonly) return;
                  event.preventDefault();
                  const optionId = event.dataTransfer.getData('text/plain');
                  if (optionId) assignOptionToSlot(slot.id, optionId);
                }}
              >
                <div className="drag-slot-label">
                  <span>{translatedNode(`interaction_target:${slot.id}`, slot.label, translationMode, translationMap)}</span>
                  {!readonly && (
                    <div className="drag-slot-actions">
                      <button type="button" disabled={!current} onClick={() => assignOptionToSlot(slot.id, null)}>
                        清空
                      </button>
                    </div>
                  )}
                </div>
                <div className="drag-slot-value">
                  <strong>{optionLabel(option, current)}</strong>
                  {props.showAnswer && (
                    <small>
                      正确：{slot.correct_option ? optionLabel(correct, slot.correct_option) : '未配置'}
                    </small>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}
