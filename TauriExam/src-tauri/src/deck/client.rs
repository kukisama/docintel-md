//! 对 opendecknew 第三方接口的 ureq HTTP 调用封装。
//!
//! 本机回环调用，使用短超时，避免设备未启动时阻塞 UI。
//! 所有函数接受 base_url / token 参数，不直接读设置（由 commands 层注入）。

use std::time::Duration;

use serde_json::{json, Value};

use super::types::{DeckEvent, DeckEvents, DeckPing, DeckSession, DeckSlotInput};

const CLIENT_NAME: &str = "TauriExam";
const DEFAULT_LEASE_MS: u64 = 6000;
const HEADER_TOKEN: &str = "X-Deck-Token";

/// 本机调用用短超时 agent，设备离线时快速失败。
fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(3))
        .build()
}

fn endpoint(base_url: &str, path: &str) -> String {
    format!("{}/{}", base_url.trim_end_matches('/'), path.trim_start_matches('/'))
}

/// 把 ureq 调用结果归一成 `Result<Value, String>`。
fn read_json(result: Result<ureq::Response, ureq::Error>) -> Result<Value, String> {
    match result {
        Ok(resp) => resp.into_json::<Value>().map_err(|err| err.to_string()),
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp.into_string().unwrap_or_default();
            Err(format!("HTTP {code}: {body}"))
        }
        Err(ureq::Error::Transport(err)) => Err(format!("无法连接设备服务：{err}")),
    }
}

/// `GET /ping` —— 无需令牌。设备不可达时返回 `reachable=false`，不报错。
pub fn ping(base_url: &str) -> DeckPing {
    let result = agent().get(&endpoint(base_url, "ping")).call();
    match read_json(result) {
        Ok(value) => DeckPing {
            reachable: true,
            enabled: value.get("enabled").and_then(Value::as_bool).unwrap_or(false),
            device_id: value
                .get("device_id")
                .and_then(Value::as_str)
                .map(str::to_string),
        },
        Err(_) => DeckPing {
            reachable: false,
            enabled: false,
            device_id: None,
        },
    }
}

/// `POST /session/takeover` —— 接管设备，拿 epoch。
pub fn takeover(base_url: &str, token: &str) -> Result<DeckSession, String> {
    let result = agent()
        .post(&endpoint(base_url, "session/takeover"))
        .set(HEADER_TOKEN, token)
        .send_json(json!({ "client": CLIENT_NAME, "lease_ms": DEFAULT_LEASE_MS }));
    let value = read_json(result)?;
    Ok(DeckSession {
        epoch: value.get("epoch").and_then(Value::as_u64).unwrap_or(0),
        event_seq: value.get("event_seq").and_then(Value::as_u64).unwrap_or(0),
        lease_ms: value
            .get("lease_ms")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_LEASE_MS),
    })
}

/// `POST /session/heartbeat` —— 续租。
pub fn heartbeat(base_url: &str, token: &str, epoch: u64) -> Result<(), String> {
    let result = agent()
        .post(&endpoint(base_url, "session/heartbeat"))
        .set(HEADER_TOKEN, token)
        .send_json(json!({ "epoch": epoch, "lease_ms": DEFAULT_LEASE_MS }));
    read_json(result).map(|_| ())
}

/// `POST /session/release` —— 释放设备。
pub fn release(base_url: &str, token: &str, epoch: u64) -> Result<(), String> {
    let result = agent()
        .post(&endpoint(base_url, "session/release"))
        .set(HEADER_TOKEN, token)
        .send_json(json!({ "epoch": epoch }));
    read_json(result).map(|_| ())
}

/// `POST /display/slots` —— 写槽位（标题/图标/颜色/热键）。
pub fn push_slots(
    base_url: &str,
    token: &str,
    epoch: u64,
    clear_first: bool,
    slots: &[DeckSlotInput],
) -> Result<(), String> {
    let result = agent()
        .post(&endpoint(base_url, "display/slots"))
        .set(HEADER_TOKEN, token)
        .send_json(json!({
            "epoch": epoch,
            "clear_first": clear_first,
            "slots": slots,
        }));
    read_json(result).map(|_| ())
}

/// `POST /display/brightness` —— 设置设备亮度（0-100）。
/// 接管期间周期性下发，可顶住 opendecknew 的空闲自动变暗，让屏幕保持工作亮度。
pub fn set_brightness(base_url: &str, token: &str, epoch: u64, brightness: u8) -> Result<(), String> {
    let result = agent()
        .post(&endpoint(base_url, "display/brightness"))
        .set(HEADER_TOKEN, token)
        .send_json(json!({ "epoch": epoch, "brightness": brightness }));
    read_json(result).map(|_| ())
}

/// `GET /events?epoch=&after=` —— 增量拉取按键事件。
pub fn poll_events(base_url: &str, token: &str, epoch: u64, after: u64) -> Result<DeckEvents, String> {
    let result = agent()
        .get(&endpoint(base_url, "events"))
        .set(HEADER_TOKEN, token)
        .query("epoch", &epoch.to_string())
        .query("after", &after.to_string())
        .call();
    let value = read_json(result)?;
    let events = value
        .get("events")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(DeckEvent {
                        seq: item.get("seq").and_then(Value::as_u64)?,
                        slot_id: item.get("slot_id").and_then(Value::as_str)?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let latest_seq = value
        .get("latest_seq")
        .and_then(Value::as_u64)
        .unwrap_or(after);
    Ok(DeckEvents { events, latest_seq })
}
