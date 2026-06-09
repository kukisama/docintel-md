//! 设备插件（StreamDock Lite / AJAZZ AKP153）后端模块。
//!
//! 本模块是 TauriExam 对 opendecknew 第三方控制接口的**消费方**封装。
//! opendecknew 暴露本机 HTTP 接口（默认 http://127.0.0.1:57200/api/v1），
//! 本模块用 ureq 调用它，并以 `#[tauri::command]` 暴露给前端。
//!
//! 设计原则：
//! - 解耦：所有设备逻辑住在 `deck/` 下，主程序仅注册命令。
//! - 可降级：设备不可达时返回"不可达"而非错误，让前端静默关闭功能。
//! - 多文件：types / settings / client / commands 各司其职，便于后续 main.rs 拆分。

mod client;
mod commands;
mod settings;
mod types;

// glob 重导出：连同 #[tauri::command] 生成的隐藏 item 一起带出，
// 供 main.rs 的 generate_handler![deck::deck_xxx] 解析。
pub use commands::*;
