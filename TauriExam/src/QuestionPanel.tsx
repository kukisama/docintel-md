import { useEffect, useRef, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { ArrowDown, ArrowUp, ChevronsLeft, ChevronsRight, Star } from 'lucide-react';
import type { DragDropUserAnswer, HotspotUserAnswer, InteractionModel, PageImage, QuestionDetail, TranslationRow } from './types';
import type { TranslationMode } from './helpers';
import { expectedLetters, optionClassName, shouldShowInteraction, createTranslationMap, translatedNode } from './helpers';
import DragDropQuestion, { createEmptyDragAnswer, gradeDragDrop, isDragDropComplete, parseDragAnswer } from './DragDropQuestion';
import HotspotQuestion, { createEmptyHotspotAnswer, gradeHotspot, isHotspotComplete, parseHotspotAnswer } from './HotspotQuestion';

function TranslationPanel(props: {
  rows: TranslationRow[];
  mode: TranslationMode;
  language: string;
  busy: boolean;
  showAnswer: boolean;
  showPages: boolean;
  pagesBusy?: boolean;
  aiEnabled?: boolean;
  aiBusy?: boolean;
  onMode: (value: TranslationMode) => void;
  onLanguage?: (value: string) => void;
  onTranslate: (force: boolean) => void;
  onPrev?: () => void;
  onNext?: () => void;
  onToggleAnswer: () => void;
  onTogglePages: () => void;
  onAskAi?: (actionType: 'analyze' | 'summarize' | 'freeform') => void;
}) {
  return (
    <section className="translation-panel">
      <div className="toolbar-row">
        <div className="segmented-control">
          <button className={props.mode === 'original' ? 'active' : ''} onClick={() => props.onMode('original')}>原文</button>
          <button className={props.mode === 'translated' ? 'active' : ''} onClick={() => props.onMode('translated')}>翻译</button>
          <button className={props.mode === 'side_by_side' ? 'active' : ''} onClick={() => props.onMode('side_by_side')}>对照</button>
        </div>
        <div className="toolbar-spacer" />
        <div className="action-row compact-actions">
          <input value={props.language} onChange={(event) => props.onLanguage?.(event.target.value)} />
          <button disabled={props.busy} onClick={() => props.onTranslate(false)} title="优先读取缓存">翻译</button>
          <button disabled={props.busy} onClick={() => props.onTranslate(true)}>重新翻译</button>
        </div>
      </div>
      <div className="toolbar-row">
        <div className="action-row compact-actions">
          <button className={props.showAnswer ? 'primary' : ''} onClick={props.onToggleAnswer}>答案</button>
          <button className={props.showPages ? 'primary' : ''} onClick={props.onTogglePages} disabled={props.pagesBusy}>
            {props.pagesBusy ? '...' : 'PDF'}
          </button>
          {props.onAskAi && (
            <>
              <button disabled={!(props.aiEnabled) || (props.aiBusy ?? false)} onClick={() => props.onAskAi!('analyze')}>
                {props.aiBusy ? '...' : 'AI分析'}
              </button>
              <button disabled={!(props.aiEnabled) || (props.aiBusy ?? false)} onClick={() => props.onAskAi!('summarize')}>
                AI总结
              </button>
            </>
          )}
          {(props.onPrev || props.onNext) && (
            <>
              <button className="icon-btn" disabled={!props.onPrev} onClick={props.onPrev} title="上一题">
                <ChevronsLeft size={20} strokeWidth={2.5} />
              </button>
              <button className="icon-btn" disabled={!props.onNext} onClick={props.onNext} title="下一题">
                <ChevronsRight size={20} strokeWidth={2.5} />
              </button>
            </>
          )}
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

function DragDropPracticePanel({ model, questionId, showAnswer, translationMode, translationMap }: { model: InteractionModel; questionId: string; showAnswer: boolean; translationMode: TranslationMode; translationMap: Map<string, string> }) {
  const [answer, setAnswer] = useState<DragDropUserAnswer>(() => createEmptyDragAnswer(model));
  const [checked, setChecked] = useState(false);
  const complete = isDragDropComplete(model, answer);
  const correct = checked ? gradeDragDrop(model, answer) : null;

  useEffect(() => {
    setAnswer(createEmptyDragAnswer(model));
    setChecked(false);
  }, [model, questionId]);

  return (
    <section className="interaction-panel">
      <h4>拖拽练习</h4>
      <div className="info-box">{model.message || '检测到结构化交互数据：请先自己拖拽作答，再检查答案。'}</div>
      <DragDropQuestion model={model} value={answer} onChange={(value) => { setAnswer(value); setChecked(false); }} showAnswer={showAnswer || checked} translationMode={translationMode} translationMap={translationMap} />
      <div className="drag-practice-actions">
        <button className="primary" disabled={!complete} onClick={() => setChecked(true)}>检查答案</button>
        <button onClick={() => { setAnswer(createEmptyDragAnswer(model)); setChecked(false); }}>重做</button>
        {!complete && <span className="muted">请先填完所有槽位。</span>}
        {correct === true && <span className="result correct">全部正确</span>}
        {correct === false && <span className="result wrong">还有槽位不正确</span>}
      </div>
    </section>
  );
}

function HotspotPracticePanel({ model, questionId, showAnswer, translationMode, translationMap }: { model: InteractionModel; questionId: string; showAnswer: boolean; translationMode: TranslationMode; translationMap: Map<string, string> }) {
  const [answer, setAnswer] = useState<HotspotUserAnswer>(() => createEmptyHotspotAnswer(model));
  const [checked, setChecked] = useState(false);
  const complete = isHotspotComplete(model, answer);
  const correct = checked ? gradeHotspot(model, answer) : null;

  useEffect(() => {
    setAnswer(createEmptyHotspotAnswer(model));
    setChecked(false);
  }, [model, questionId]);

  return (
    <section className="interaction-panel">
      <h4>Hotspot 下拉练习</h4>
      <div className="info-box">{model.message || '检测到结构化交互数据：请先选择每个下拉框，再检查答案。'}</div>
      <HotspotQuestion model={model} value={answer} onChange={(value) => { setAnswer(value); setChecked(false); }} showAnswer={showAnswer || checked} translationMode={translationMode} translationMap={translationMap} />
      <div className="drag-practice-actions">
        <button className="primary" disabled={!complete} onClick={() => setChecked(true)}>检查答案</button>
        <button onClick={() => { setAnswer(createEmptyHotspotAnswer(model)); setChecked(false); }}>重做</button>
        {!complete && <span className="muted">请先完成所有下拉选择。</span>}
        {correct === true && <span className="result correct">全部正确</span>}
        {correct === false && <span className="result wrong">还有选择不正确</span>}
      </div>
    </section>
  );
}

function InteractionPanel({ model, showAnswer }: { model: InteractionModel; showAnswer: boolean }) {
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
              <strong>{showAnswer ? row.correct_selection || row.option_group || '待选择' : row.option_group || '待作答'}</strong>
            </div>
          ))}
        </div>
      )}
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

export default function QuestionPanel(props: {
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
  dragAnswer?: DragDropUserAnswer | null;
  hotspotAnswer?: HotspotUserAnswer | null;
  historyUserAnswer?: string | null;
  onOptionSelect?: (key: string) => void;
  onDragAnswerChange?: (value: DragDropUserAnswer) => void;
  onHotspotAnswerChange?: (value: HotspotUserAnswer) => void;
  onReturn?: () => void;
  onToggleFlag?: (flagType: string) => void;
  onAiPrompt?: (value: string) => void;
  onAskAi?: (actionType: 'analyze' | 'summarize' | 'freeform') => void;
  onTranslationMode?: (value: TranslationMode) => void;
  onTranslationLanguage?: (value: string) => void;
  onTranslate?: (force: boolean) => void;
  onToggleAnswer: () => void;
  onTogglePages: () => void;
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
  const isStructuredDrag = props.interaction?.kind === 'drag_drop' && props.interaction.slots.length > 0;
  const isStructuredHotspot = props.interaction?.kind === 'hotspot' && props.interaction.rows.length > 0 && props.interaction.options.length > 0;
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

      {!props.compact && props.translationMode && props.onTranslationMode && props.onTranslate && (
        <TranslationPanel
          rows={props.translations || []}
          mode={props.translationMode}
          language={props.translationLanguage || 'zh-CN'}
          busy={props.translationBusy ?? false}
          showAnswer={props.showAnswer}
          showPages={props.pages.length > 0}
          pagesBusy={props.pagesBusy}
          aiEnabled={props.aiEnabled}
          aiBusy={props.aiBusy}
          onMode={props.onTranslationMode}
          onLanguage={props.onTranslationLanguage}
          onTranslate={props.onTranslate}
          onPrev={props.onPrev}
          onNext={props.onNext}
          onToggleAnswer={props.onToggleAnswer}
          onTogglePages={props.onTogglePages}
          onAskAi={props.onAskAi}
        />
      )}

      <section>
        <h4>题目</h4>
        <div className="text-block">{translatedNode('question_text', detail.question_text, translationMode, translationMap)}</div>
      </section>

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

      {!props.hideAnswerAreas && detail.answer_areas.length > 0 && props.showAnswer && (
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

      {props.interaction?.kind === 'drag_drop' && props.interaction.slots.length > 0 && props.compact && (
        <DragDropQuestion model={props.interaction} value={props.dragAnswer} onChange={props.onDragAnswerChange} translationMode={translationMode} translationMap={translationMap} />
      )}

      {isStructuredHotspot && props.compact && (
        <HotspotQuestion model={props.interaction!} value={props.hotspotAnswer} onChange={props.onHotspotAnswerChange} translationMode={translationMode} translationMap={translationMap} />
      )}

      {props.interaction?.kind === 'drag_drop' && props.interaction.slots.length > 0 && !props.compact && props.historyUserAnswer && (
        <DragDropQuestion model={props.interaction} value={parseDragAnswer(props.historyUserAnswer)} showAnswer disabled translationMode={translationMode} translationMap={translationMap} />
      )}

      {isStructuredHotspot && !props.compact && props.historyUserAnswer && (
        <HotspotQuestion model={props.interaction!} value={parseHotspotAnswer(props.historyUserAnswer)} showAnswer disabled translationMode={translationMode} translationMap={translationMap} />
      )}

      {props.interaction?.kind === 'drag_drop' && props.interaction.slots.length > 0 && !props.compact && !props.historyUserAnswer && (
        <DragDropPracticePanel model={props.interaction} questionId={detail.id} showAnswer={props.showAnswer} translationMode={translationMode} translationMap={translationMap} />
      )}

      {isStructuredHotspot && !props.compact && !props.historyUserAnswer && (
        <HotspotPracticePanel model={props.interaction!} questionId={detail.id} showAnswer={props.showAnswer} translationMode={translationMode} translationMap={translationMap} />
      )}

      {!props.compact && shouldShowInteraction(props.interaction) && !(props.interaction?.kind === 'drag_drop' && props.interaction.slots.length > 0) && !isStructuredHotspot && <InteractionPanel model={props.interaction!} showAnswer={props.showAnswer} />}

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
