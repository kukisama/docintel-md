// useExamDeck：设备生命周期 hook。检测不到设备就完全 no-op。
//
// 职责：
//   - 进入考试视图且已配置时探测设备 → 接管 → 心跳 → 推送当前题。
//   - 题目/选择变化时重绘设备。
//   - 离开视图 / 卸载时释放设备。
// 不承担任何答题逻辑：用户按设备键由 opendecknew 注入热键，命中现有键盘监听。

import { useEffect, useRef } from 'react';
import type { QuestionDetail } from '../types';
import type { DeckSlotInput } from '../types';
import * as deck from './deckClient';
import { buildSlots, interpretSlot, isDeckSupported, type DeckExamContext } from './deckLayout';

const HEARTBEAT_MS = 2500;
/** 设备离线时的重探间隔，避免频繁打扰。 */
const REPROBE_MS = 15000;
/** 按键事件轮询间隔。 */
const EVENTS_MS = 250;
/** 亮度默认值，对齐 DeckHelper 出厂默认 50（未传入时的兑底）。 */
const DEFAULT_BRIGHTNESS = 50;

export type UseExamDeckParams = {
  /** 是否处于需要投屏的场景（如考试视图）。 */
  active: boolean;
  detail: QuestionDetail | null;
  selected: string[];
  index: number;
  total: number;
  /** 是否启用翻题键（仅在宿主界面 ←/→ 已接线时）。 */
  navEnabled: boolean;
  hasPrev: boolean;
  hasNext: boolean;
  /** 本地兜底亮度（0-100）：仅当读不到 DeckHelper 主程序亮度时使用。取自设置，未传则用默认 50。 */
  brightness?: number;
  /** 设备按选项键：选中/取消该选项。 */
  onOption?: (letter: string) => void;
  /** 设备按提交键。 */
  onSubmit?: () => void;
  /** 设备按上一题（navEnabled 为 true 时才有意义）。 */
  onPrev?: () => void;
  /** 设备按下一题。 */
  onNext?: () => void;
};

export function useExamDeck(params: UseExamDeckParams): void {
  const epochRef = useRef<number | null>(null);
  const heartbeatTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  const eventsTimer = useRef<ReturnType<typeof setInterval> | null>(null);
  /** 已消费的最大事件 seq，只处理比它新的按键。 */
  const afterSeqRef = useRef(0);
  /** 上一次每个槽位推送的快照（slot_id -> 序列化），用于只推变化的槽位。 */
  const lastSlotsRef = useRef<Map<string, string>>(new Map());
  /** 下一次 render 是否需要整页重画（刚接管时）。 */
  const fullRedrawRef = useRef(true);
  /** DeckHelper 主程序当前亮度（读 state-db.json），优先于本地设置值；读不到为 null。 */
  const hostBrightnessRef = useRef<number | null>(null);
  // 最新参数快照，供定时器/异步流程读取，避免闭包过期。
  const paramsRef = useRef(params);
  paramsRef.current = params;

  // 接管 / 释放：仅依赖 active，避免每次切题重连。
  useEffect(() => {
    let cancelled = false;
    let reprobeTimer: ReturnType<typeof setTimeout> | null = null;

    async function start() {
      if (cancelled) return;
      const p = paramsRef.current;
      if (!p.active || !isDeckSupported(p.detail)) {
        scheduleReprobe();
        return;
      }
      const probe = await deck.ping();
      if (cancelled) return;
      if (!probe.reachable || !probe.enabled) {
        scheduleReprobe();
        return;
      }
      const session = await deck.takeover();
      if (cancelled || !session) {
        scheduleReprobe();
        return;
      }
      epochRef.current = session.epoch;
      afterSeqRef.current = session.event_seq;
      // 刚接管：优先读 DeckHelper 主程序亮度并据此点亮；清空快照并要求整页重画。
      await refreshHostBrightness();
      if (cancelled) return;
      void deck.setBrightness(session.epoch, brightnessValue());
      lastSlotsRef.current.clear();
      fullRedrawRef.current = true;
      startHeartbeat();
      startEventsPoll();
      void render();
    }

    function startHeartbeat() {
      stopHeartbeat();
      heartbeatTimer.current = setInterval(async () => {
        const epoch = epochRef.current;
        if (epoch == null) return;
        const ok = await deck.heartbeat(epoch);
        if (!ok && !cancelled) {
          // 会话失效：清理并尝试重连。
          teardown(false);
          scheduleReprobe();
          return;
        }
        // 顺带刷新主程序亮度，与 DeckHelper 设置保持同步。
        await refreshHostBrightness();
        // 重新下发亮度，顶住 opendecknew 空闲自动变暗。
        void deck.setBrightness(epoch, brightnessValue());
      }, HEARTBEAT_MS);
    }

    function stopHeartbeat() {
      if (heartbeatTimer.current) {
        clearInterval(heartbeatTimer.current);
        heartbeatTimer.current = null;
      }
    }

    function startEventsPoll() {
      stopEventsPoll();
      eventsTimer.current = setInterval(async () => {
        const epoch = epochRef.current;
        if (epoch == null) return;
        const result = await deck.pollEvents(epoch, afterSeqRef.current);
        if (cancelled) return;
        for (const ev of result.events) {
          if (ev.seq > afterSeqRef.current) afterSeqRef.current = ev.seq;
          dispatch(ev.slot_id);
        }
        if (result.latest_seq > afterSeqRef.current) afterSeqRef.current = result.latest_seq;
      }, EVENTS_MS);
    }

    function stopEventsPoll() {
      if (eventsTimer.current) {
        clearInterval(eventsTimer.current);
        eventsTimer.current = null;
      }
    }

    /** 把一次按键翻译成答题动作并回调到宿主。 */
    function dispatch(slotId: string) {
      const p = paramsRef.current;
      if (!p.active || !p.detail || !isDeckSupported(p.detail)) return;
      const action = interpretSlot(slotId, p.detail, p.navEnabled);
      if (!action) return;
      switch (action.kind) {
        case 'option':
          p.onOption?.(action.letter);
          break;
        case 'submit':
          p.onSubmit?.();
          break;
        case 'prev':
          p.onPrev?.();
          break;
        case 'next':
          p.onNext?.();
          break;
      }
    }

    function scheduleReprobe() {
      if (cancelled) return;
      if (reprobeTimer) clearTimeout(reprobeTimer);
      reprobeTimer = setTimeout(start, REPROBE_MS);
    }

    /** 读 DeckHelper 主程序当前亮度，缓存到 ref；读不到置 null（后续回退本地设置值）。 */
    async function refreshHostBrightness() {
      const v = await deck.hostBrightness();
      if (cancelled) return;
      hostBrightnessRef.current =
        typeof v === 'number' && !Number.isNaN(v) ? Math.max(0, Math.min(100, Math.round(v))) : null;
    }

    /** 取生效亮度（0-100）：优先用 DeckHelper 主程序设置值，读不到才用本地设置，再不行用默认。 */
    function brightnessValue(): number {
      const host = hostBrightnessRef.current;
      if (typeof host === 'number' && !Number.isNaN(host)) {
        return Math.max(0, Math.min(100, Math.round(host)));
      }
      const v = paramsRef.current.brightness;
      if (typeof v !== 'number' || Number.isNaN(v)) return DEFAULT_BRIGHTNESS;
      return Math.max(0, Math.min(100, Math.round(v)));
    }

    function teardown(doRelease: boolean) {
      stopHeartbeat();
      stopEventsPoll();
      const epoch = epochRef.current;
      epochRef.current = null;
      if (doRelease && epoch != null) void deck.release(epoch);
    }

    void start();

    return () => {
      cancelled = true;
      if (reprobeTimer) clearTimeout(reprobeTimer);
      teardown(true);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [params.active]);

  // 题目 / 选择变化时重绘当前页。
  useEffect(() => {
    void render();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [params.detail?.id, params.selected.join(','), params.index, params.total]);

  async function render() {
    const epoch = epochRef.current;
    const p = paramsRef.current;
    if (epoch == null || !p.active || !p.detail || !isDeckSupported(p.detail)) return;
    const ctx: DeckExamContext = {
      detail: p.detail,
      selected: p.selected,
      index: p.index,
      total: p.total,
      navEnabled: p.navEnabled,
      hasPrev: p.hasPrev,
      hasNext: p.hasNext,
    };
    const next = buildSlots(ctx);
    const nextMap = new Map<string, string>();
    for (const s of next) nextMap.set(s.slot_id, JSON.stringify(s));

    if (fullRedrawRef.current) {
      // 首次/接管后：整页重画一次，后续走差量。
      const ok = await deck.pushSlots(epoch, true, next);
      if (ok) {
        lastSlotsRef.current = nextMap;
        fullRedrawRef.current = false;
      }
      return;
    }

    // 差量刷新：只推变化的槽位 + 清除消失的槽位。
    const prevMap = lastSlotsRef.current;
    const changed: typeof next = [];
    for (const s of next) {
      if (prevMap.get(s.slot_id) !== nextMap.get(s.slot_id)) changed.push(s);
    }
    const removed: DeckSlotInput[] = [];
    for (const slotId of prevMap.keys()) {
      if (!nextMap.has(slotId)) removed.push({ slot_id: slotId, clear: true });
    }
    if (changed.length === 0 && removed.length === 0) return;
    const ok = await deck.pushSlots(epoch, false, [...changed, ...removed]);
    if (ok) lastSlotsRef.current = nextMap;
  }
}
