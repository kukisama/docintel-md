import { useEffect, useState } from 'react';
import { BookOpen, FolderOpen, Gamepad2, RefreshCw, Settings, ShieldCheck } from 'lucide-react';
import type { AiSettings, AppPaths, BatchTranslateEvent, BankHealth, BankInfo, DeckPing, DeckSettings } from './types';
import { Stat } from './QuestionList';

const DEFAULT_DECK_SETTINGS: DeckSettings = {
  enabled: false,
  base_url: 'http://127.0.0.1:57200/api/v1',
  token: '',
  brightness: 50,
};

export default function SettingsPanel(props: {
  appPaths: AppPaths | null;
  banks: BankInfo[];
  bankId: string;
  bankHealth: BankHealth | null;
  aiSettings: AiSettings | null;
  deckSettings: DeckSettings | null;
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
  onSaveDeckSettings: (settings: DeckSettings) => void;
  onTestDeckConnection: () => Promise<DeckPing>;
}) {
  const currentBank = props.banks.find((bank) => bank.id === props.bankId);
  const [tab, setTab] = useState<'files' | 'ai' | 'translation' | 'device'>('files');
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

  const [deckDraft, setDeckDraft] = useState<DeckSettings>(props.deckSettings || DEFAULT_DECK_SETTINGS);
  const [deckProbe, setDeckProbe] = useState<DeckPing | null>(null);
  const [deckProbing, setDeckProbing] = useState(false);

  useEffect(() => {
    if (props.deckSettings) setDeckDraft(props.deckSettings);
  }, [props.deckSettings]);

  async function probeDeck() {
    setDeckProbing(true);
    try {
      setDeckProbe(await props.onTestDeckConnection());
    } catch {
      setDeckProbe({ reachable: false, enabled: false, device_id: null });
    } finally {
      setDeckProbing(false);
    }
  }

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
        <button className={tab === 'device' ? 'active' : ''} onClick={() => setTab('device')}>
          <Gamepad2 size={16} /> 设备
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

      {tab === 'device' && (
        <div className="settings-tab-body">
          <div className="panel settings-card">
            <div className="settings-head">
              <h3>外接设备（StreamDock / AKP153）</h3>
              <label className="switch-label">
                <input
                  type="checkbox"
                  checked={deckDraft.enabled}
                  onChange={(e) => setDeckDraft({ ...deckDraft, enabled: e.target.checked })}
                />
                启用设备支持
              </label>
            </div>
            <p className="muted">
              检测到本机运行 opendecknew 时，考试中会把当前题投到设备，按设备键即可选答 / 提交。未检测到设备时不影响任何功能。
            </p>
            <table className="file-table">
              <tbody>
                <tr>
                  <td className="ft-label">接口地址</td>
                  <td>
                    <input
                      className="text-input"
                      value={deckDraft.base_url}
                      placeholder="http://127.0.0.1:57200/api/v1"
                      onChange={(e) => setDeckDraft({ ...deckDraft, base_url: e.target.value })}
                    />
                  </td>
                </tr>
                <tr>
                  <td className="ft-label">令牌 Token</td>
                  <td>
                    <input
                      className="text-input"
                      type="password"
                      value={deckDraft.token}
                      placeholder="从 opendecknew 设置页复制"
                      onChange={(e) => setDeckDraft({ ...deckDraft, token: e.target.value })}
                    />
                  </td>
                </tr>
                <tr>
                  <td className="ft-label">亮度 0-100</td>
                  <td>
                    <input
                      className="text-input"
                      type="number"
                      min={0}
                      max={100}
                      value={deckDraft.brightness}
                      placeholder="DeckHelper 默认 50"
                      onChange={(e) =>
                        setDeckDraft({
                          ...deckDraft,
                          brightness: Math.max(0, Math.min(100, Math.round(Number(e.target.value) || 0))),
                        })
                      }
                    />
                  </td>
                </tr>
              </tbody>
            </table>
            <div className="action-row compact-actions">
              <button className="primary" onClick={() => props.onSaveDeckSettings(deckDraft)}>
                保存
              </button>
              <button onClick={probeDeck} disabled={deckProbing}>
                {deckProbing ? '检测中...' : '检测设备'}
              </button>
            </div>
            {deckProbe && (
              <div className={deckProbe.reachable && deckProbe.enabled ? 'info-box ok' : 'info-box'}>
                {deckProbe.reachable
                  ? deckProbe.enabled
                    ? `设备在线：${deckProbe.device_id || '未知型号'}`
                    : 'opendecknew 在线，但第三方控制能力已关闭（请在其设置页开启）。'
                  : '未检测到 opendecknew。请确认它已运行，且地址正确。'}
              </div>
            )}
          </div>
        </div>
      )}
    </section>
  );
}
