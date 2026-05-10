import { useMemo } from 'react';
import type { HotspotUserAnswer, InteractionModel } from './types';
import type { TranslationMode } from './helpers';
import { translatedNode } from './helpers';

export function createEmptyHotspotAnswer(model: InteractionModel): HotspotUserAnswer {
  return {
    kind: 'hotspot',
    rows: model.rows.map((row) => ({ row_id: row.id, option_id: null })),
  };
}

export function createCorrectHotspotAnswer(model: InteractionModel): HotspotUserAnswer {
  return {
    kind: 'hotspot',
    rows: model.rows.map((row) => ({ row_id: row.id, option_id: row.correct_selection ?? null })),
  };
}

export function normalizeHotspotAnswer(model: InteractionModel, value?: HotspotUserAnswer | null): HotspotUserAnswer {
  const existing = new Map((value?.rows || []).map((row) => [row.row_id, row.option_id]));
  return {
    kind: 'hotspot',
    rows: model.rows.map((row) => ({ row_id: row.id, option_id: existing.get(row.id) ?? null })),
  };
}

export function parseHotspotAnswer(value?: string | null): HotspotUserAnswer | null {
  if (!value) return null;
  try {
    const parsed = JSON.parse(value) as Partial<HotspotUserAnswer>;
    if (parsed?.kind !== 'hotspot' || !Array.isArray(parsed.rows)) return null;
    return {
      kind: 'hotspot',
      rows: parsed.rows
        .filter((row) => typeof row?.row_id === 'string')
        .map((row) => ({ row_id: row.row_id, option_id: typeof row.option_id === 'string' ? row.option_id : null })),
    };
  } catch {
    return null;
  }
}

export function isHotspotComplete(model: InteractionModel, answer?: HotspotUserAnswer | null) {
  const normalized = normalizeHotspotAnswer(model, answer);
  return model.rows.length > 0 && normalized.rows.every((row) => Boolean(row.option_id));
}

export function gradeHotspot(model: InteractionModel, answer: HotspotUserAnswer): boolean {
  const rowMap = new Map(answer.rows.map((row) => [row.row_id, row.option_id]));
  return model.rows.length > 0 && model.rows.every((row) => rowMap.get(row.id) === row.correct_selection);
}

export function describeHotspotAnswer(model: InteractionModel, answer: HotspotUserAnswer) {
  const rowMap = new Map(answer.rows.map((row) => [row.row_id, row.option_id]));
  return model.rows.map((row) => `${row.id}=${rowMap.get(row.id) || '未选'}`).join('; ');
}

export default function HotspotQuestion(props: {
  model: InteractionModel;
  value?: HotspotUserAnswer | null;
  showAnswer?: boolean;
  translationMode?: TranslationMode;
  translationMap?: Map<string, string>;
  disabled?: boolean;
  onChange?: (value: HotspotUserAnswer) => void;
}) {
  const answer = useMemo(() => normalizeHotspotAnswer(props.model, props.value), [props.model, props.value]);
  const optionById = useMemo(() => new Map(props.model.options.map((option) => [option.key, option])), [props.model.options]);
  const readonly = props.disabled || !props.onChange;
  const translationMode = props.translationMode || 'original';
  const translationMap = props.translationMap || new Map<string, string>();

  function optionsForGroup(group?: string | null) {
    return props.model.options.filter((option) => !group || option.group === group);
  }

  function updateRow(rowId: string, optionId: string | null) {
    if (readonly) return;
    props.onChange?.({
      kind: 'hotspot',
      rows: answer.rows.map((row) => (row.row_id === rowId ? { ...row, option_id: optionId } : row)),
    });
  }

  function translatedText(fieldName: string, original: string) {
    const translated = translationMap.get(`${fieldName}:0`);
    return translationMode === 'original' || !translated ? original : translated;
  }

  return (
    <section className="hotspot-board">
      <div className="drag-drop-head">
        <div>
          <h4>Hotspot 下拉作答</h4>
          <p>{readonly ? '只读模式：查看你的选择和正确答案。' : '为每个空位选择一个下拉答案。'}</p>
        </div>
        <span>{isHotspotComplete(props.model, answer) ? '已填完' : '未填完'}</span>
      </div>
      <div className="hotspot-row-list">
        {props.model.rows.map((row) => {
          const current = answer.rows.find((item) => item.row_id === row.id)?.option_id ?? null;
          const currentOption = optionById.get(current || '');
          const correctOption = row.correct_selection ? optionById.get(row.correct_selection) : undefined;
          const isCorrect = Boolean(current && row.correct_selection && current === row.correct_selection);
          const isWrong = Boolean(props.showAnswer && current && row.correct_selection && current !== row.correct_selection);
          return (
            <div className={['hotspot-row-card', props.showAnswer && isCorrect ? 'correct' : '', isWrong ? 'wrong' : ''].filter(Boolean).join(' ')} key={row.id}>
              <label>{translatedNode(`interaction_target:${row.id}`, row.prompt, translationMode, translationMap)}</label>
              <select disabled={readonly} value={current || ''} onChange={(event) => updateRow(row.id, event.target.value || null)}>
                <option value="">请选择...</option>
                {optionsForGroup(row.option_group).map((option) => (
                  <option key={option.key} value={option.key}>{translatedText(`interaction_option:${option.key}`, option.text)}</option>
                ))}
              </select>
              {props.showAnswer && (
                <small>
                  正确：{correctOption ? translatedNode(`interaction_option:${correctOption.key}`, correctOption.text, translationMode, translationMap) : row.correct_selection || '未配置'}
                  {currentOption && current !== row.correct_selection && (
                    <>；你的选择：{translatedNode(`interaction_option:${currentOption.key}`, currentOption.text, translationMode, translationMap)}</>
                  )}
                </small>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}
