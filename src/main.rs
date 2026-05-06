use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug)]
struct Config {
    input: PathBuf,
    output: Option<PathBuf>,
    endpoint: String,
    key: String,
    cloud: String,
    api_version: String,
    model: String,
    figure_mode: FigureMode,
    poll_seconds: u64,
    timeout_seconds: u64,
}

#[derive(Debug, Clone, Copy)]
enum FigureMode {
    Inline,
    Ignore,
    Separate,
}

impl FigureMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "inline" | "with-images" | "with-figures" => Ok(Self::Inline),
            "ignore" | "none" | "no-images" | "no-figures" => Ok(Self::Ignore),
            "separate" | "manifest" | "figures-file" => Ok(Self::Separate),
            other => Err(format!(
                "unsupported --figure-mode value: {other}; use inline, ignore, or separate"
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Inline => "inline",
            Self::Ignore => "ignore",
            Self::Separate => "separate",
        }
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let env_values = load_env_values();
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return Ok(());
    }

    let args = if args.first().map(String::as_str) == Some("analyze") {
        &args[1..]
    } else {
        &args[..]
    };

    let flags = parse_flags(args)?;
    let config = build_config(flags, env_values)?;
    validate_cloud_endpoint(&config.cloud, &config.endpoint)?;
    analyze(config)
}

fn print_help() {
    println!(
        "docintel-md\n\nUsage:\n  docintel-md analyze --input <file> [--output <dir>] [--endpoint <url>] [--key <key>] [--cloud global|21v] [--figure-mode inline|ignore|separate]\n\nConfig can come from .env in the current directory, .env next to the exe, or environment variables:\n  DOCINTEL_ENDPOINT\n  DOCINTEL_KEY\n  DOCINTEL_CLOUD\n  DOCINTEL_API_VERSION\n  DOCINTEL_MODEL\n  DOCINTEL_FIGURE_MODE\n\nFigure modes:\n  inline    Keep figure OCR blocks in the main Markdown\n  ignore    Remove figure OCR blocks from the main Markdown\n  separate  Remove figure OCR blocks from the main Markdown and write them to *.figures.md\n\nDefaults:\n  --cloud global\n  --api-version 2024-11-30\n  --model prebuilt-layout\n  --figure-mode separate\n  --poll-seconds 1\n  --timeout-seconds 300"
    );
}

fn parse_flags(args: &[String]) -> Result<HashMap<String, String>, String> {
    let mut flags = HashMap::new();
    let mut index = 0;
    while index < args.len() {
        let key = &args[index];
        if !key.starts_with("--") {
            return Err(format!("unexpected argument: {key}"));
        }
        let name = key.trim_start_matches("--").to_string();
        index += 1;
        if index >= args.len() || args[index].starts_with("--") {
            return Err(format!("missing value for --{name}"));
        }
        flags.insert(name, args[index].clone());
        index += 1;
    }
    Ok(flags)
}

fn load_env_values() -> HashMap<String, String> {
    let mut values = HashMap::new();

    if let Ok(current_dir) = env::current_dir() {
        read_dotenv_upwards(&current_dir, &mut values);
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            read_dotenv_upwards(exe_dir, &mut values);
        }
    }

    for key in [
        "DOCINTEL_ENDPOINT",
        "DOCINTEL_KEY",
        "DOCINTEL_CLOUD",
        "DOCINTEL_API_VERSION",
        "DOCINTEL_MODEL",
        "DOCINTEL_FIGURE_MODE",
    ] {
        if let Ok(value) = env::var(key) {
            if !value.trim().is_empty() {
                values.insert(key.to_string(), value);
            }
        }
    }

    values
}

fn read_dotenv_upwards(start_dir: &Path, values: &mut HashMap<String, String>) {
    for dir in start_dir.ancestors() {
        read_dotenv(&dir.join(".env"), values);
    }
}

fn read_dotenv(path: &Path, values: &mut HashMap<String, String>) {
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_matches('"').trim_matches('\'');
        if !key.is_empty() && !value.is_empty() {
            values.insert(key.to_string(), value.to_string());
        }
    }
}

fn build_config(
    flags: HashMap<String, String>,
    env_values: HashMap<String, String>,
) -> Result<Config, String> {
    let input = flags
        .get("input")
        .map(PathBuf::from)
        .ok_or("missing --input <file>")?;
    if !input.exists() {
        return Err(format!("input not found: {}", input.display()));
    }

    let endpoint = get_value(&flags, &env_values, "endpoint", "DOCINTEL_ENDPOINT")
        .ok_or("missing --endpoint or DOCINTEL_ENDPOINT")?;
    let key = get_value(&flags, &env_values, "key", "DOCINTEL_KEY")
        .ok_or("missing --key or DOCINTEL_KEY")?;
    let figure_mode = get_value(&flags, &env_values, "figure-mode", "DOCINTEL_FIGURE_MODE")
        .map(|value| FigureMode::parse(&value))
        .transpose()?
        .unwrap_or(FigureMode::Separate);

    Ok(Config {
        input,
        output: flags.get("output").map(PathBuf::from),
        endpoint,
        key,
        cloud: get_value(&flags, &env_values, "cloud", "DOCINTEL_CLOUD")
            .unwrap_or_else(|| "global".to_string()),
        api_version: get_value(&flags, &env_values, "api-version", "DOCINTEL_API_VERSION")
            .unwrap_or_else(|| "2024-11-30".to_string()),
        model: get_value(&flags, &env_values, "model", "DOCINTEL_MODEL")
            .unwrap_or_else(|| "prebuilt-layout".to_string()),
        figure_mode,
        poll_seconds: flags
            .get("poll-seconds")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1),
        timeout_seconds: flags
            .get("timeout-seconds")
            .and_then(|v| v.parse().ok())
            .unwrap_or(300),
    })
}

fn get_value(
    flags: &HashMap<String, String>,
    env_values: &HashMap<String, String>,
    flag_name: &str,
    env_name: &str,
) -> Option<String> {
    flags
        .get(flag_name)
        .cloned()
        .or_else(|| env_values.get(env_name).cloned())
        .filter(|v| !v.trim().is_empty())
}

fn validate_cloud_endpoint(cloud: &str, endpoint: &str) -> Result<(), String> {
    let endpoint_lower = endpoint.to_ascii_lowercase();
    match cloud.to_ascii_lowercase().as_str() {
        "global" => {
            if endpoint_lower.contains(".azure.cn") {
                return Err(
                    "--cloud global does not match an Azure China endpoint (.azure.cn)".to_string(),
                );
            }
        }
        "21v" | "china" => {
            if !endpoint_lower.contains(".azure.cn") {
                return Err(
                    "--cloud 21v expects an Azure China endpoint ending in .azure.cn".to_string(),
                );
            }
        }
        other => {
            return Err(format!(
                "unsupported --cloud value: {other}; use global or 21v"
            ))
        }
    }
    Ok(())
}

fn analyze(config: Config) -> Result<(), String> {
    let output_dir = config
        .output
        .clone()
        .unwrap_or_else(|| default_output_dir(&config.input));
    fs::create_dir_all(&output_dir)
        .map_err(|e| format!("failed to create output dir {}: {e}", output_dir.display()))?;

    let stem = config
        .input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let markdown_path = output_dir.join(format!("{stem}.document-intelligence.md"));
    let json_path = output_dir.join(format!("{stem}.document-intelligence.json"));
    let figures_path = output_dir.join(format!("{stem}.document-intelligence.figures.md"));
    let meta_path = output_dir.join(format!("{stem}.document-intelligence.meta.json"));
    let readme_path = output_dir.join("README.md");

    let mut bytes = Vec::new();
    fs::File::open(&config.input)
        .map_err(|e| format!("failed to open {}: {e}", config.input.display()))?
        .read_to_end(&mut bytes)
        .map_err(|e| format!("failed to read {}: {e}", config.input.display()))?;

    let content_type = content_type_for(&config.input)?;
    let endpoint = config.endpoint.trim_end_matches('/');
    let analyze_url = format!(
        "{endpoint}/documentintelligence/documentModels/{}:analyze?api-version={}&outputContentFormat=markdown",
        config.model, config.api_version
    );

    println!(
        "Submitting {} to Document Intelligence...",
        config.input.display()
    );
    let started = Instant::now();
    let response = ureq::post(&analyze_url)
        .set("Ocp-Apim-Subscription-Key", &config.key)
        .set("Content-Type", content_type)
        .send_bytes(&bytes)
        .map_err(format_ureq_error)?;

    let operation_location = response
        .header("Operation-Location")
        .ok_or("response missing Operation-Location header")?
        .to_string();

    let payload = poll_result(
        &operation_location,
        &config.key,
        config.poll_seconds,
        config.timeout_seconds,
    )?;
    let pretty = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("failed to serialize JSON: {e}"))?;
    fs::write(&json_path, pretty + "\n")
        .map_err(|e| format!("failed to write {}: {e}", json_path.display()))?;

    let markdown = payload
        .pointer("/analyzeResult/content")
        .and_then(Value::as_str)
        .unwrap_or("");
    let readable_markdown = match config.figure_mode {
        FigureMode::Inline => render_figures_as_code_blocks(markdown),
        FigureMode::Ignore | FigureMode::Separate => strip_figures_from_markdown(markdown),
    };
    fs::write(
        &markdown_path,
        readable_markdown.trim_end().to_string() + "\n",
    )
    .map_err(|e| format!("failed to write {}: {e}", markdown_path.display()))?;

    let figure_count = figure_count(&payload);
    let figures_markdown = if matches!(config.figure_mode, FigureMode::Separate) {
        write_figures_md(&figures_path, &payload)?;
        Some(
            figures_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default(),
        )
    } else {
        if figures_path.exists() {
            fs::remove_file(&figures_path)
                .map_err(|e| format!("failed to remove stale {}: {e}", figures_path.display()))?;
        }
        None
    };

    let elapsed_ms = started.elapsed().as_millis();
    let meta = json!({
        "input": config.input.display().to_string(),
        "endpoint_host": endpoint_host(&config.endpoint),
        "cloud": config.cloud,
        "model": config.model,
        "api_version": config.api_version,
        "content_type": content_type,
        "output_content_format": "markdown",
        "figure_mode": config.figure_mode.as_str(),
        "elapsed_ms": elapsed_ms,
        "markdown": markdown_path.file_name().and_then(|s| s.to_str()).unwrap_or_default(),
        "json": json_path.file_name().and_then(|s| s.to_str()).unwrap_or_default(),
        "figures_markdown": figures_markdown,
        "figure_count": figure_count
    });
    fs::write(
        &meta_path,
        serde_json::to_string_pretty(&meta).unwrap() + "\n",
    )
    .map_err(|e| format!("failed to write {}: {e}", meta_path.display()))?;

    write_readme(
        &readme_path,
        &config.input,
        &markdown_path,
        &json_path,
        figures_markdown.map(|_| figures_path.as_path()),
        &meta_path,
        config.figure_mode,
    )?;

    println!("Wrote {}", markdown_path.display());
    println!("Wrote {}", json_path.display());
    if matches!(config.figure_mode, FigureMode::Separate) {
        println!("Wrote {}", figures_path.display());
    }
    println!("Wrote {}", meta_path.display());
    Ok(())
}

fn figure_count(payload: &Value) -> usize {
    payload
        .pointer("/analyzeResult/figures")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

fn write_figures_md(path: &Path, payload: &Value) -> Result<usize, String> {
    let content = payload
        .pointer("/analyzeResult/content")
        .and_then(Value::as_str)
        .unwrap_or("");
    let figures = payload
        .pointer("/analyzeResult/figures")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    let mut lines = vec![
        "# 图片 OCR 清单".to_string(),
        "".to_string(),
        "本文件把 Azure Document Intelligence 识别到的图片/图块 OCR 文本从主 Markdown 中拆出来。".to_string(),
        "生成最终 `.2nd.md` 时，可把它当作图片文字转录或图片描述素材；不要把噪声 OCR 原样全部合并进正文。".to_string(),
        "".to_string(),
        format!("- 检测到的图片/图块数量：{}", figures.len()),
        "".to_string(),
    ];

    for (index, figure) in figures.iter().enumerate() {
        let page = figure
            .pointer("/boundingRegions/0/pageNumber")
            .and_then(Value::as_i64)
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let offset = figure
            .pointer("/spans/0/offset")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        let length = figure
            .pointer("/spans/0/length")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        let text = clean_figure_text(&substring_by_chars(content, offset, length));

        lines.push(format!("## 图片/图块 {}", index + 1));
        lines.push("".to_string());
        lines.push(format!("- 页码：{}", page));
        lines.push(format!("- OCR 字符数：{}", text.chars().count()));
        lines.push("".to_string());
        lines.push("```text".to_string());
        if text.is_empty() {
            lines.push("<no OCR text>".to_string());
        } else {
            lines.push(text);
        }
        lines.push("```".to_string());
        lines.push("".to_string());
    }

    fs::write(path, lines.join("\n"))
        .map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    Ok(figures.len())
}

fn render_figures_as_code_blocks(markdown: &str) -> String {
    let mut output = String::with_capacity(markdown.len());
    let mut rest = markdown;
    let mut figure_index = 1usize;

    while let Some(start) = rest.find("<figure>") {
        let (before, after_start) = rest.split_at(start);
        output.push_str(before);
        let after_open = &after_start["<figure>".len()..];

        if let Some(end) = after_open.find("</figure>") {
            let raw_figure = &after_open[..end];
            let figure_text = clean_figure_text(raw_figure);
            output.push_str(&format!(
                "\n\n> Figure {} OCR transcript\n\n```text\n{}\n```\n\n",
                figure_index,
                if figure_text.is_empty() {
                    "<no OCR text>".to_string()
                } else {
                    figure_text
                }
            ));
            figure_index += 1;
            rest = &after_open[end + "</figure>".len()..];
        } else {
            output.push_str(after_start);
            return output;
        }
    }

    output.push_str(rest);
    output
}

fn strip_figures_from_markdown(markdown: &str) -> String {
    let mut output = String::with_capacity(markdown.len());
    let mut rest = markdown;

    while let Some(start) = rest.find("<figure>") {
        let (before, after_start) = rest.split_at(start);
        output.push_str(before);
        let after_open = &after_start["<figure>".len()..];

        if let Some(end) = after_open.find("</figure>") {
            rest = &after_open[end + "</figure>".len()..];
        } else {
            output.push_str(after_start);
            return output;
        }
    }

    output.push_str(rest);
    output
}

fn substring_by_chars(content: &str, offset: usize, length: usize) -> String {
    content.chars().skip(offset).take(length).collect()
}

fn clean_figure_text(text: &str) -> String {
    text.replace("<figure>", "")
        .replace("</figure>", "")
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn poll_result(
    operation_location: &str,
    key: &str,
    poll_seconds: u64,
    timeout_seconds: u64,
) -> Result<Value, String> {
    let started = Instant::now();
    loop {
        let response = ureq::get(operation_location)
            .set("Ocp-Apim-Subscription-Key", key)
            .call()
            .map_err(format_ureq_error)?;
        let response_text = response
            .into_string()
            .map_err(|e| format!("failed to read service response: {e}"))?;
        let payload: Value = serde_json::from_str(&response_text)
            .map_err(|e| format!("failed to parse service JSON: {e}"))?;
        let status = payload
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        match status {
            "succeeded" => return Ok(payload),
            "failed" | "canceled" => return Err(format!("analysis {status}: {}", payload)),
            _ => {
                if started.elapsed() > Duration::from_secs(timeout_seconds) {
                    return Err(format!(
                        "analysis timed out after {timeout_seconds}s; last status: {status}"
                    ));
                }
                println!("status: {status}; polling again in {poll_seconds}s...");
                std::thread::sleep(Duration::from_secs(poll_seconds));
            }
        }
    }
}

fn default_output_dir(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    PathBuf::from("output")
        .join("document-intelligence")
        .join(stem)
}

fn content_type_for(path: &Path) -> Result<&'static str, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "pdf" => Ok("application/pdf"),
        "png" => Ok("image/png"),
        "jpg" | "jpeg" => Ok("image/jpeg"),
        "tif" | "tiff" => Ok("image/tiff"),
        "bmp" => Ok("image/bmp"),
        _ => Err(format!(
            "unsupported input extension: .{ext}; use pdf/png/jpg/jpeg/tif/tiff/bmp"
        )),
    }
}

fn endpoint_host(endpoint: &str) -> String {
    endpoint
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .split('/')
        .next()
        .unwrap_or(endpoint)
        .to_string()
}

fn write_readme(
    readme_path: &Path,
    input: &Path,
    markdown_path: &Path,
    json_path: &Path,
    figures_path: Option<&Path>,
    meta_path: &Path,
    figure_mode: FigureMode,
) -> Result<(), String> {
    let figure_line = figures_path
        .map(|path| {
            format!(
                "- 图片 OCR 清单：`{}`\n",
                path.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("document-intelligence.figures.md")
            )
        })
        .unwrap_or_default();
    let figure_mode_note = match figure_mode {
        FigureMode::Inline => "图片/图块 OCR 已内联写入主 Markdown。",
        FigureMode::Ignore => {
            "图片/图块 OCR 已从主 Markdown 中移除；完整原始结果仍保留在 JSON 中。"
        }
        FigureMode::Separate => "图片/图块 OCR 已从主 Markdown 中移除，并单独写入图片 OCR 清单。",
    };
    let content = format!(
        "# Document Intelligence 输出文件\n\n源文件：`{}`\n\n- 主 Markdown：`{}`\n- 原始 JSON：`{}`\n{}- 元数据：`{}`\n\n图片处理模式：`{}`。{}\n\n建议：主 Markdown 适合直接给 AI 阅读；JSON 保留表格、页码、版面和置信度等完整数据；如果生成了图片 OCR 清单，可把它作为图片文字转录或描述素材。\n",
        input.file_name().and_then(|s| s.to_str()).unwrap_or("document"),
        markdown_path.file_name().and_then(|s| s.to_str()).unwrap_or("document-intelligence.md"),
        json_path.file_name().and_then(|s| s.to_str()).unwrap_or("document-intelligence.json"),
        figure_line,
        meta_path.file_name().and_then(|s| s.to_str()).unwrap_or("document-intelligence.meta.json"),
        figure_mode.as_str(),
        figure_mode_note
    );
    fs::write(readme_path, content)
        .map_err(|e| format!("failed to write {}: {e}", readme_path.display()))
}

fn format_ureq_error(err: ureq::Error) -> String {
    match err {
        ureq::Error::Status(code, response) => {
            let text = response
                .into_string()
                .unwrap_or_else(|_| "<failed to read response body>".to_string());
            format!("service returned HTTP {code}: {text}")
        }
        ureq::Error::Transport(transport) => format!("transport error: {transport}"),
    }
}
