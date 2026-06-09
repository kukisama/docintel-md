//! 设备插件的数据结构。前后端通过这些类型交换数据。

use serde::{Deserialize, Serialize};

/// 设备连接设置，持久化在 app_settings 表（key 前缀 `deck.`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeckSettings {
    /// 是否启用设备支持。关闭时前端不会接管设备。
    pub enabled: bool,
    /// opendecknew 接口基址，如 http://127.0.0.1:57200/api/v1。
    pub base_url: String,
    /// 鉴权令牌，从 opendecknew 设置页复制。空则功能关闭。
    pub token: String,
    /// 接管期间维持的设备亮度（0-100）。默认 50，对齐 DeckHelper 出厂默认。
    #[serde(default = "default_brightness")]
    pub brightness: u8,
}

fn default_brightness() -> u8 {
    50
}

/// `/ping` 探测结果。设备不可达时 `reachable=false`，而非返回 Err。
#[derive(Debug, Serialize)]
pub struct DeckPing {
    pub reachable: bool,
    pub enabled: bool,
    pub device_id: Option<String>,
}

/// `takeover` 成功后的会话信息。
#[derive(Debug, Serialize)]
pub struct DeckSession {
    pub epoch: u64,
    pub event_seq: u64,
    pub lease_ms: u64,
}

/// 写给某个槽位的内容。`None` 字段不会序列化到 opendecknew 请求体。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeckSlotInput {
    pub slot_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// PNG 图标的 base64（裸 base64 或 data:image/png;base64,...）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 代发热键，如 "a" / "left" / "enter"。用户按下时由 opendecknew 注入前台窗口。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emit_key: Option<String>,
    /// 为 true 时清空该格。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clear: Option<bool>,
}

/// 单个按键按下事件。
#[derive(Debug, Serialize)]
pub struct DeckEvent {
    pub seq: u64,
    pub slot_id: String,
}

/// `/events` 拉取结果。
#[derive(Debug, Serialize)]
pub struct DeckEvents {
    pub events: Vec<DeckEvent>,
    pub latest_seq: u64,
}
