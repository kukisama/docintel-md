//! AI 推理与翻译的底层调用层。
//!
//! - OpenAI 兼容 `responses` 接口（普通 + 流式）。
//! - 微软 Translator Text REST v3 与 AI 翻译两条后端。
//! - AI 设置的加载与默认值。
//! 上层命令在 `translation` 模块，本模块只负责"怎么发请求、怎么解析"。

use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::time::Duration;

use crate::models::{AiSettings, QuestionDetail, TranslationSegment};
use crate::storage::{get_setting, open_app_db};

pub(crate) const DEFAULT_PROMPT_ANALYZE: &str = "请用中文详细分析这道考试题。要求：1) 解释题干问什么；2) 解释正确答案为什么正确；3) 分析每个错误选项为什么错；4) 提炼知识点；5) 给出记忆方法。";
pub(crate) const DEFAULT_PROMPT_SUMMARIZE: &str = "请用中文简洁总结这道考试题。要求：1) 一句话概括题目在问什么；2) 正确答案是什么；3) 核心考点是什么；4) 关键词列表。不需要逐选项分析。";

fn default_ai_settings() -> AiSettings {
    AiSettings {
        enabled: false,
        base_url: "https://api.openai.com/v1".to_string(),
        api_version: String::new(),
        api_key: String::new(),
        model: "gpt-4.1-mini".to_string(),
        temperature: 0.7,
        system_prompt: String::new(),
        prompt_analyze: DEFAULT_PROMPT_ANALYZE.to_string(),
        prompt_summarize: DEFAULT_PROMPT_SUMMARIZE.to_string(),
        translation_provider: "ai".to_string(),
        translator_endpoint: "https://api.cognitive.microsofttranslator.com".to_string(),
        translator_key: String::new(),
        translator_region: String::new(),
    }
}

pub(crate) fn load_ai_settings() -> Result<AiSettings, String> {
    let conn = open_app_db()?;
    let defaults = default_ai_settings();
    let mut settings = AiSettings {
        enabled: get_setting(&conn, "ai.enabled")?.map(|value| value == "true").unwrap_or(defaults.enabled),
        base_url: get_setting(&conn, "ai.base_url")?.unwrap_or(defaults.base_url),
        api_version: get_setting(&conn, "ai.api_version")?.unwrap_or(defaults.api_version),
        api_key: get_setting(&conn, "ai.api_key")?.unwrap_or(defaults.api_key),
        model: get_setting(&conn, "ai.model")?.unwrap_or(defaults.model),
        temperature: get_setting(&conn, "ai.temperature")?
            .and_then(|value| value.parse::<f32>().ok())
            .unwrap_or(defaults.temperature),
        system_prompt: get_setting(&conn, "ai.system_prompt")?.unwrap_or(defaults.system_prompt),
        prompt_analyze: get_setting(&conn, "ai.prompt_analyze")?.unwrap_or(defaults.prompt_analyze),
        prompt_summarize: get_setting(&conn, "ai.prompt_summarize")?.unwrap_or(defaults.prompt_summarize),
        translation_provider: get_setting(&conn, "translation.provider")?.unwrap_or(defaults.translation_provider),
        translator_endpoint: get_setting(&conn, "translator.endpoint")?.unwrap_or(defaults.translator_endpoint),
        translator_key: get_setting(&conn, "translator.key")?.unwrap_or(defaults.translator_key),
        translator_region: get_setting(&conn, "translator.region")?.unwrap_or(defaults.translator_region),
    };
    if settings.api_version.trim().is_empty() {
        settings.api_version = effective_ai_api_version(&settings);
    }
    Ok(settings)
}

pub(crate) fn question_context(question: &QuestionDetail) -> String {
    let options = question
        .options
        .iter()
        .map(|option| format!("{}. {}", option.option_key, option.option_text))
        .collect::<Vec<_>>()
        .join("\n");
    let answer_areas = question
        .answer_areas
        .iter()
        .map(|row| format!("{} => {}", row.prompt, row.recommended_selection))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "题型: {}\nTopic: {}\n页码: {}\n\n题干:\n{}\n\n选项:\n{}\n\n答案区:\n{}\n\n源答案: {}\n推荐答案: {}\n中文判断: {}\nReasoning:\n{}\nNotes:\n{}",
        question.question_type,
        question.topic.clone().unwrap_or_default(),
        question.source_pages.clone().unwrap_or_default(),
        question.question_text,
        options,
        answer_areas,
        question.source_answer.clone().unwrap_or_default(),
        question.recommended_answer.clone().unwrap_or_default(),
        question.chinese_judgement.clone().unwrap_or_default(),
        question.reasoning.clone().unwrap_or_default(),
        question.notes.clone().unwrap_or_default()
    )
}

pub(crate) fn call_responses_api(settings: &AiSettings, prompt: &str) -> Result<String, String> {
    if !settings.enabled {
        return Err("AI 未启用，请先在控制面板启用 AI。".to_string());
    }
    if settings.api_key.trim().is_empty() {
        return Err("AI API Key 为空，请先在控制面板填写。".to_string());
    }
    let api_version = effective_ai_api_version(settings);
    let url = if api_version.is_empty() {
        format!("{}/responses", settings.base_url.trim_end_matches('/'))
    } else {
        format!(
            "{}/responses?api-version={}",
            settings.base_url.trim_end_matches('/'),
            api_version
        )
    };
    let agent = http_agent();
    let mut body = json!({
        "model": settings.model,
        "temperature": settings.temperature,
        "input": prompt
    });
    if !settings.system_prompt.trim().is_empty() {
        body["instructions"] = json!(settings.system_prompt.trim());
    }
    let mut request = agent.post(&url);
    request = if api_version.is_empty() {
        request.set("Authorization", &format!("Bearer {}", settings.api_key.trim()))
    } else {
        request.set("api-key", settings.api_key.trim())
    };
    let response = request
        .send_json(&body)
        .map_err(|err| err.to_string())?;
    let status = response.status();
    let response_body: Value = response.into_json().map_err(|err| err.to_string())?;
    if status < 200 || status >= 300 {
        return Err(format!("AI 请求失败 ({status}): {response_body}"));
    }
    extract_response_text(&response_body).ok_or_else(|| format!("AI 响应中未找到文本内容: {response_body}"))
}

pub(crate) fn call_responses_api_stream<F>(settings: &AiSettings, prompt: &str, mut on_delta: F) -> Result<String, String>
where
    F: FnMut(&str) -> Result<(), String>,
{
    if !settings.enabled {
        return Err("AI 未启用，请先在控制面板启用 AI。".to_string());
    }
    if settings.api_key.trim().is_empty() {
        return Err("AI API Key 为空，请先在控制面板填写。".to_string());
    }
    let api_version = effective_ai_api_version(settings);
    let url = if api_version.is_empty() {
        format!("{}/responses", settings.base_url.trim_end_matches('/'))
    } else {
        format!(
            "{}/responses?api-version={}",
            settings.base_url.trim_end_matches('/'),
            api_version
        )
    };
    let agent = http_agent();
    let mut body = json!({
        "model": settings.model,
        "temperature": settings.temperature,
        "input": prompt,
        "stream": true
    });
    if !settings.system_prompt.trim().is_empty() {
        body["instructions"] = json!(settings.system_prompt.trim());
    }
    let mut request = agent
        .post(&url)
        .set("Accept", "text/event-stream")
        .set("Cache-Control", "no-cache");
    request = if api_version.is_empty() {
        request.set("Authorization", &format!("Bearer {}", settings.api_key.trim()))
    } else {
        request.set("api-key", settings.api_key.trim())
    };
    let response = request.send_json(&body).map_err(|err| err.to_string())?;
    let status = response.status();
    if status < 200 || status >= 300 {
        let body = response.into_string().unwrap_or_default();
        return Err(format!("AI 请求失败 ({status}): {body}"));
    }
    let mut content = String::new();
    let reader = BufReader::new(response.into_reader());
    for line in reader.lines() {
        let line = line.map_err(|err| err.to_string())?;
        let Some(data) = line.strip_prefix("data:") else {
            if content.is_empty() {
                if let Ok(value) = serde_json::from_str::<Value>(&line) {
                    if let Some(text) = extract_response_text(&value) {
                        content.push_str(&text);
                        on_delta(&text)?;
                    }
                }
            }
            continue;
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let value: Value = match serde_json::from_str(data) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if let Some(delta) = extract_stream_delta(&value) {
            content.push_str(&delta);
            on_delta(&delta)?;
        }
    }
    Ok(content)
}

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(120))
        .build()
}

fn extract_stream_delta(value: &Value) -> Option<String> {
    if value.get("type").and_then(Value::as_str) == Some("response.output_text.delta") {
        return value.get("delta").and_then(Value::as_str).map(str::to_string);
    }
    value.get("delta").and_then(Value::as_str).map(str::to_string)
}

fn effective_ai_api_version(settings: &AiSettings) -> String {
    let configured = settings.api_version.trim();
    if !configured.is_empty() {
        return configured.to_string();
    }
    let base_url = settings.base_url.to_lowercase();
    if (base_url.contains("azure-api.net") || base_url.contains(".openai.azure.com")) && !base_url.contains("/v1") {
        return "2025-03-01-preview".to_string();
    }
    String::new()
}

fn extract_response_text(value: &Value) -> Option<String> {
    if let Some(text) = value.get("output_text").and_then(Value::as_str) {
        if !text.trim().is_empty() {
            return Some(text.to_string());
        }
    }
    let mut parts = Vec::new();
    if let Some(output) = value.get("output").and_then(Value::as_array) {
        for item in output {
            if let Some(content) = item.get("content").and_then(Value::as_array) {
                for part in content {
                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        parts.push(text.to_string());
                    }
                }
            }
        }
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn translator_language(language: &str) -> String {
    match language.trim().to_lowercase().as_str() {
        "zh-cn" | "zh_cn" | "zh" => "zh-Hans".to_string(),
        "zh-tw" | "zh_tw" | "zh-hk" | "zh_hk" => "zh-Hant".to_string(),
        value if value.is_empty() => "zh-Hans".to_string(),
        value => value.to_string(),
    }
}

pub(crate) fn call_translator_batch_api(settings: &AiSettings, segments: &[TranslationSegment], language: &str) -> Result<Vec<String>, String> {
    if settings.translator_key.trim().is_empty() {
        return Err("Microsoft Translator Key 为空，请先在控制面板填写。".to_string());
    }
    let key_diagnostics = translator_key_diagnostics(&settings.translator_key);
    if !key_diagnostics.looks_like_azure_key {
        return Err(format!("Microsoft Translator Key 看起来不是有效 Azure key（{}）。请重新粘贴 Azure 门户 Keys and Endpoint 里的 Key1 或 Key2。", key_diagnostics.summary()));
    }
    if segments.is_empty() {
        return Ok(Vec::new());
    }
    let target = translator_language(language);
    let translator_region = translator_region_for_config(settings)?;
    let url = translator_url_with_target("https://api.cognitive.microsofttranslator.com/translate", &target);
    let agent = http_agent();
    let body = segments
        .iter()
        .map(|segment| json!({ "Text": segment.source_text }))
        .collect::<Vec<_>>();
    let response = agent
        .post(&url)
        .set("Ocp-Apim-Subscription-Key", settings.translator_key.trim())
        .set("Ocp-Apim-Subscription-Region", translator_region.as_str())
        .set("Content-Type", "application/json; charset=UTF-8")
        .send_json(&json!(body))
        .map_err(|err| err.to_string())?;
    let status = response.status();
    let response_text = response.into_string().map_err(|err| err.to_string())?;
    let response_body: Value = serde_json::from_str(&response_text)
        .map_err(|err| format!("Microsoft Translator 响应不是合法 JSON ({status}): {err}; 原文: {response_text}"))?;
    if status < 200 || status >= 300 {
        return Err(format!("Microsoft Translator 请求失败 ({status})，已按官方 Text Translation REST 方式请求 https://api.cognitive.microsofttranslator.com/translate，并传入 Ocp-Apim-Subscription-Key 与 Ocp-Apim-Subscription-Region；key 诊断：{}；响应: {response_body}", key_diagnostics.summary()));
    }
    parse_translator_response(&response_body)
}

struct TranslatorKeyDiagnostics {
    len: usize,
    is_ascii: bool,
    has_password_bullets: bool,
    has_inner_whitespace: bool,
    has_key_label_chars: bool,
    only_token_chars: bool,
    looks_like_azure_key: bool,
}

impl TranslatorKeyDiagnostics {
    fn summary(&self) -> String {
        format!(
            "len={}, ascii={}, contains_password_bullets={}, azure_key_shape={}",
            self.len, self.is_ascii, self.has_password_bullets, self.looks_like_azure_key
        ) + &format!(
            ", inner_whitespace={}, label_chars={}, token_chars_only={}",
            self.has_inner_whitespace, self.has_key_label_chars, self.only_token_chars
        )
    }
}

fn translator_key_diagnostics(key: &str) -> TranslatorKeyDiagnostics {
    let trimmed = key.trim();
    let len = trimmed.chars().count();
    let is_ascii = trimmed.is_ascii();
    let has_password_bullets = trimmed.contains('•') || trimmed.contains('●') || trimmed.contains('*');
    let has_inner_whitespace = trimmed.chars().any(char::is_whitespace);
    let has_key_label_chars = trimmed.contains(':');
    let only_token_chars = trimmed.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '+' || ch == '/' || ch == '=' || ch == '-' || ch == '_');
    let looks_like_azure_key = len >= 20 && len <= 128 && is_ascii && !has_password_bullets && !has_inner_whitespace && !has_key_label_chars && only_token_chars;
    TranslatorKeyDiagnostics { len, is_ascii, has_password_bullets, has_inner_whitespace, has_key_label_chars, only_token_chars, looks_like_azure_key }
}

fn translator_url_with_target(base: &str, target: &str) -> String {
    let separator = if base.contains('?') { '&' } else { '?' };
    let mut url = format!("{base}{separator}");
    if !base.to_lowercase().contains("api-version=") {
        url.push_str("api-version=3.0&");
    }
    url.push_str("to=");
    url.push_str(target);
    url
}

fn translator_region_for_config(settings: &AiSettings) -> Result<String, String> {
    let region = settings.translator_region.trim();
    if region.is_empty() {
        return Err("Microsoft Translator Region 为空；请填写 Azure 门户“位置/区域”里的资源区域，例如 swedencentral。".to_string());
    }
    if !is_supported_translator_endpoint(&settings.translator_endpoint) {
        return Err("Microsoft Translator Endpoint 格式不支持；当前只支持官方 Text Translation endpoint：https://api.cognitive.microsofttranslator.com/。".to_string());
    }
    Ok(region.to_string())
}

fn is_supported_translator_endpoint(endpoint: &str) -> bool {
    let host = normalized_endpoint_host(endpoint);
    host == "api.cognitive.microsofttranslator.com"
}

fn normalized_endpoint_host(endpoint: &str) -> String {
    let endpoint = endpoint.trim().trim_start_matches("https://").trim_start_matches("http://");
    endpoint.split('/').next().unwrap_or_default().to_lowercase()
}

fn parse_translator_response(body: &Value) -> Result<Vec<String>, String> {
    let items = body.as_array().ok_or_else(|| format!("Microsoft Translator 响应不是数组: {body}"))?;
    let mut translations = Vec::new();
    for item in items {
        let text = item
            .get("translations")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|translation| translation.get("text"))
            .and_then(Value::as_str)
            .ok_or_else(|| format!("Microsoft Translator 响应中未找到译文: {body}"))?;
        translations.push(text.to_string());
    }
    Ok(translations)
}

#[cfg(test)]
fn sanitized_translator_url_path(url: &str) -> String {
    let without_scheme = url.split_once("://").map(|(_, right)| right).unwrap_or(url);
    let (host, path) = without_scheme.split_once('/').unwrap_or((without_scheme, ""));
    let host_label = if host.eq_ignore_ascii_case("api.cognitive.microsofttranslator.com") {
        "global-translator"
    } else if host.ends_with(".api.cognitive.microsoft.com") {
        "regional-cognitive"
    } else if host.ends_with(".cognitiveservices.azure.com") {
        "custom-cognitiveservices"
    } else {
        "configured-endpoint"
    };
    format!("{host_label}/{path}")
}

pub(crate) fn call_ai_translation_api(settings: &AiSettings, segments: &[TranslationSegment], language: &str) -> Result<Vec<String>, String> {
    if segments.is_empty() {
        return Ok(Vec::new());
    }
    let items = segments
        .iter()
        .map(|segment| json!({
            "field_name": segment.field_name,
            "segment_index": segment.segment_index,
            "text": segment.source_text
        }))
        .collect::<Vec<_>>();
    let prompt = format!(
        "请把下面考试题页面的结构化 JSON 翻译成 {language}。要求：\n1. 必须只输出 JSON，不要 Markdown，不要解释。\n2. JSON 顶层必须是数组。\n3. 每个元素必须保留 field_name 和 segment_index 原值，并输出 translated_text。\n4. 保留 Microsoft 产品名、考试术语、选项字母、URL、代码、专有名词；不要改答案字母。\n5. 不要遗漏任何元素，输出顺序与输入一致。\n\n输入 JSON：\n{}",
        serde_json::to_string(&items).map_err(|err| err.to_string())?
    );
    let content = call_responses_api(settings, &prompt)?;
    parse_ai_translation_json(&content, segments.len())
}

fn parse_ai_translation_json(content: &str, expected_len: usize) -> Result<Vec<String>, String> {
    let trimmed = content.trim();
    let json_text = if trimmed.starts_with("```") {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    } else {
        trimmed
    };
    let value: Value = serde_json::from_str(json_text).map_err(|err| format!("AI 翻译结果不是合法 JSON: {err}; 原文: {content}"))?;
    let items = value.as_array().ok_or_else(|| format!("AI 翻译结果顶层不是数组: {value}"))?;
    if items.len() != expected_len {
        return Err(format!("AI 翻译结果数量不匹配：期望 {expected_len}，实际 {}。", items.len()));
    }
    items
        .iter()
        .map(|item| {
            item.get("translated_text")
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| format!("AI 翻译结果缺少 translated_text: {item}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "uses saved live Microsoft Translator credentials"]
    fn translator_live_hello_with_saved_settings() {
        let settings = load_ai_settings().expect("load saved app settings");
        assert!(
            !settings.translator_key.trim().is_empty(),
            "translator key is empty in saved settings"
        );
        assert!(
            !settings.translator_endpoint.trim().is_empty(),
            "translator endpoint is empty in saved settings"
        );
        let region = translator_region_for_config(&settings).expect("resolve translator region");
        let url = translator_url_with_target("https://api.cognitive.microsofttranslator.com/translate", "zh-Hans");
        println!("translator endpoint shape: {}", sanitized_translator_endpoint_shape(&settings.translator_endpoint));
        println!("translator region: {region}");
        println!("translator key edge: {}", test_key_edge(&settings.translator_key));
        println!("translator request path: {}", super::sanitized_translator_url_path(&url));
        let result = call_translator_batch_api(
            &settings,
            &[TranslationSegment {
                field_name: "probe".to_string(),
                segment_index: 0,
                source_text: "Hello".to_string(),
            }],
            "zh-CN",
        )
        .expect("translate Hello with saved Microsoft Translator settings");
        assert_eq!(result.len(), 1);
        assert!(
            result[0].contains('你') || result[0].contains('好') || result[0].to_lowercase() != "hello",
            "unexpected translation result: {}",
            result[0]
        );
        println!("translator live result: {}", result[0]);
    }

    #[test]
    #[ignore = "uses saved live Microsoft Translator credentials"]
    fn translator_live_official_text_rest_with_saved_settings() {
        let mut settings = load_ai_settings().expect("load saved app settings");
        if let Ok(endpoint) = std::env::var("TRANSLATOR_TEST_ENDPOINT") {
            settings.translator_endpoint = endpoint;
        }
        if let Ok(region) = std::env::var("TRANSLATOR_TEST_REGION") {
            settings.translator_region = region;
        }
        assert!(
            !settings.translator_key.trim().is_empty(),
            "translator key is empty in saved settings"
        );
        let region = translator_region_for_config(&settings).expect("resolve translator region");
        let url = translator_url_with_target("https://api.cognitive.microsofttranslator.com/translate", "zh-Hans");
        let agent = super::http_agent();
        let body = json!([{ "Text": "Hello" }]);

        println!("official text endpoint shape: {}", sanitized_translator_endpoint_shape(&settings.translator_endpoint));
        println!("official text region: {region}");
        println!("official text key edge: {}", test_key_edge(&settings.translator_key));
        println!("official text url: {}", super::sanitized_translator_url_path(&url));

        let direct = agent
            .post(&url)
            .set("Ocp-Apim-Subscription-Key", settings.translator_key.trim())
            .set("Ocp-Apim-Subscription-Region", region.as_str())
            .set("Content-Type", "application/json; charset=UTF-8")
            .send_json(&body)
            .expect("send official key+region request");
        let direct_status = direct.status();
        let direct_text = direct.into_string().expect("read official key+region response");
        println!("official key+region status: {direct_status}");
        println!("official key+region body: {direct_text}");

        assert!(
            direct_status >= 200 && direct_status < 300,
            "official Translator Text REST key+region request failed; status={direct_status}"
        );
    }

    fn sanitized_translator_endpoint_shape(endpoint: &str) -> &'static str {
        let lower = endpoint.to_lowercase();
        if lower.contains(".cognitiveservices.azure.com") {
            "custom-cognitiveservices"
        } else if lower.contains(".cognitive.microsofttranslator.com") {
            "translator-global-or-regional"
        } else {
            "other"
        }
    }

    fn test_key_edge(key: &str) -> String {
        let trimmed = key.trim();
        let prefix = trimmed.chars().take(2).collect::<String>();
        let suffix = trimmed.chars().rev().take(2).collect::<Vec<_>>().into_iter().rev().collect::<String>();
        format!("{prefix}...{suffix} (len={})", trimmed.chars().count())
    }
}
