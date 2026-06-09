// 设备插件前端客户端：对 api.deck* 的薄封装。
//
// 设计要点：所有调用都"静默失败"——设备不存在 / 未启动 / 被占用时，
// 这里把错误吞掉并返回安全默认值，绝不向上抛，保证主程序答题流程不受影响。

import { api } from '../api';
import type { DeckEvents, DeckPing, DeckSession, DeckSlotInput } from '../types';

/** 探测设备是否在线。任何异常都视为不可达。 */
export async function ping(): Promise<DeckPing> {
  try {
    return await api.deckPing();
  } catch {
    return { reachable: false, enabled: false, device_id: null };
  }
}

/** 接管设备。失败返回 null（调用方据此放弃本轮接管）。 */
export async function takeover(): Promise<DeckSession | null> {
  try {
    return await api.deckTakeover();
  } catch {
    return null;
  }
}

/** 续租。返回是否成功；失败说明会话已失效。 */
export async function heartbeat(epoch: number): Promise<boolean> {
  try {
    await api.deckHeartbeat(epoch);
    return true;
  } catch {
    return false;
  }
}

/** 推送槽位内容。返回是否成功。 */
export async function pushSlots(
  epoch: number,
  clearFirst: boolean,
  slots: DeckSlotInput[],
): Promise<boolean> {
  try {
    await api.deckPushSlots(epoch, clearFirst, slots);
    return true;
  } catch {
    return false;
  }
}

/** 设置设备亮度（0-100），顺带顶住空闲变暗。失败静默。 */
export async function setBrightness(epoch: number, brightness: number): Promise<void> {
  try {
    await api.deckSetBrightness(epoch, brightness);
  } catch {
    // 忽略：亮度不关键，不影响答题。
  }
}

/** 读取 DeckHelper 主程序当前亮度设置（0-100）。读不到返回 null。失败静默。 */
export async function hostBrightness(): Promise<number | null> {
  try {
    return await api.deckHostBrightness();
  } catch {
    return null;
  }
}

/** 拉取按键事件。失败返回空结果。 */
export async function pollEvents(epoch: number, after: number): Promise<DeckEvents> {
  try {
    return await api.deckPollEvents(epoch, after);
  } catch {
    return { events: [], latest_seq: after };
  }
}

/** 释放设备。失败也不报错（opendecknew 有租约超时兜底）。 */
export async function release(epoch: number): Promise<void> {
  try {
    await api.deckRelease(epoch);
  } catch {
    // 忽略：设备侧会因心跳超时自动回收。
  }
}
