//! 设备插件暴露给前端的 Tauri 命令。薄层：读设置 → 调 client。

use super::client;
use super::settings;
use super::types::{DeckEvents, DeckPing, DeckSession, DeckSettings, DeckSlotInput};

#[tauri::command]
pub fn deck_get_settings() -> Result<DeckSettings, String> {
    settings::load()
}

#[tauri::command]
pub fn deck_save_settings(settings: DeckSettings) -> Result<DeckSettings, String> {
    settings::save(&settings)
}

/// 探测设备是否在线。无需令牌；不可达时返回 `reachable=false`，不报错。
#[tauri::command]
pub fn deck_ping() -> Result<DeckPing, String> {
    let cfg = settings::load()?;
    Ok(client::ping(&cfg.base_url))
}

#[tauri::command]
pub fn deck_takeover() -> Result<DeckSession, String> {
    let cfg = settings::load()?;
    ensure_usable(&cfg)?;
    client::takeover(&cfg.base_url, &cfg.token)
}

#[tauri::command]
pub fn deck_heartbeat(epoch: u64) -> Result<(), String> {
    let cfg = settings::load()?;
    ensure_usable(&cfg)?;
    client::heartbeat(&cfg.base_url, &cfg.token, epoch)
}

#[tauri::command]
pub fn deck_push_slots(
    epoch: u64,
    clear_first: bool,
    slots: Vec<DeckSlotInput>,
) -> Result<(), String> {
    let cfg = settings::load()?;
    ensure_usable(&cfg)?;
    client::push_slots(&cfg.base_url, &cfg.token, epoch, clear_first, &slots)
}

#[tauri::command]
pub fn deck_set_brightness(epoch: u64, brightness: u8) -> Result<(), String> {
    let cfg = settings::load()?;
    ensure_usable(&cfg)?;
    client::set_brightness(&cfg.base_url, &cfg.token, epoch, brightness)
}

/// 读取 DeckHelper 主程序当前的亮度设置（appSettings.brightness）。
/// 读不到时返回 `None`，前端回退到本地设置的亮度值。无需令牌、不报错。
#[tauri::command]
pub fn deck_host_brightness() -> Option<u8> {
    settings::read_host_brightness()
}

#[tauri::command]
pub fn deck_poll_events(epoch: u64, after: u64) -> Result<DeckEvents, String> {
    let cfg = settings::load()?;
    ensure_usable(&cfg)?;
    client::poll_events(&cfg.base_url, &cfg.token, epoch, after)
}

#[tauri::command]
pub fn deck_release(epoch: u64) -> Result<(), String> {
    let cfg = settings::load()?;
    ensure_usable(&cfg)?;
    client::release(&cfg.base_url, &cfg.token, epoch)
}

/// 控制类命令的前置校验：启用 + 有令牌。
fn ensure_usable(cfg: &DeckSettings) -> Result<(), String> {
    if !cfg.enabled {
        return Err("设备支持未启用。".to_string());
    }
    if cfg.token.trim().is_empty() {
        return Err("未配置设备令牌。".to_string());
    }
    Ok(())
}
