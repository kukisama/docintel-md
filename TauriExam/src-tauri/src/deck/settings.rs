//! 设备插件的设置读写。复用主程序的 app_settings 表（key 前缀 `deck.`）。

use std::path::PathBuf;

use crate::{get_setting, open_app_db, set_setting};

use super::types::DeckSettings;

/// DeckHelper 主程序的应用数据目录标识（Tauri identifier）。
const HOST_APP_DIR: &str = "dev.deckhelper.rs";
/// DeckHelper 持久化的本地状态库文件名（内含 appSettings.brightness）。
const HOST_STATE_DB: &str = "state-db.json";

const KEY_ENABLED: &str = "deck.enabled";
const KEY_BASE_URL: &str = "deck.base_url";
const KEY_TOKEN: &str = "deck.token";
const KEY_BRIGHTNESS: &str = "deck.brightness";

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:57200/api/v1";
/// 默认亮度，对齐 DeckHelper 出厂默认值 50。
const DEFAULT_BRIGHTNESS: u8 = 50;

/// 读取设备设置，未配置时返回默认值（关闭、默认地址、空令牌）。
pub fn load() -> Result<DeckSettings, String> {
    let conn = open_app_db()?;
    Ok(DeckSettings {
        enabled: get_setting(&conn, KEY_ENABLED)?
            .map(|value| value == "true")
            .unwrap_or(false),
        base_url: get_setting(&conn, KEY_BASE_URL)?
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
        token: get_setting(&conn, KEY_TOKEN)?.unwrap_or_default(),
        brightness: get_setting(&conn, KEY_BRIGHTNESS)?
            .and_then(|value| value.trim().parse::<u8>().ok())
            .map(|value| value.min(100))
            .unwrap_or(DEFAULT_BRIGHTNESS),
    })
}

/// 写入设备设置并回读。
pub fn save(settings: &DeckSettings) -> Result<DeckSettings, String> {
    let conn = open_app_db()?;
    set_setting(&conn, KEY_ENABLED, if settings.enabled { "true" } else { "false" })?;
    let base_url = normalize_base_url(&settings.base_url);
    set_setting(&conn, KEY_BASE_URL, &base_url)?;
    set_setting(&conn, KEY_TOKEN, settings.token.trim())?;
    set_setting(&conn, KEY_BRIGHTNESS, &settings.brightness.min(100).to_string())?;
    drop(conn);
    load()
}

/// 去掉结尾斜杠；为空时退回默认地址。
fn normalize_base_url(value: &str) -> String {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        DEFAULT_BASE_URL.to_string()
    } else {
        trimmed.to_string()
    }
}

/// DeckHelper 本地状态库（state-db.json）的绝对路径。
/// Windows 下位于 `%APPDATA%\dev.deckhelper.rs\state-db.json`。
fn host_state_db_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(base).join(HOST_APP_DIR).join(HOST_STATE_DB))
}

/// 读取 DeckHelper 主程序当前设置的亮度（appSettings.brightness，0-100）。
/// 读不到文件 / 解析失败 / 字段缺失时返回 None，由调用方回退到本地设置值。
pub fn read_host_brightness() -> Option<u8> {
    let path = host_state_db_path()?;
    let content = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;
    let brightness = value.get("appSettings")?.get("brightness")?.as_u64()?;
    Some(brightness.min(100) as u8)
}
