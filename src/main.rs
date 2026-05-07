use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
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
    split_pages: Option<u32>,
}

#[derive(Debug, Clone)]
struct PdfChunk {
    path: PathBuf,
    first_page: u32,
    last_page: u32,
    total_pages: usize,
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

    if args.first().map(String::as_str) == Some("split") {
        return split_command(&args[1..]);
    }

    if args.first().map(String::as_str) == Some("segment") {
        return segment_command(&args[1..]);
    }

    if args.first().map(String::as_str) == Some("extract-json") {
        return extract_json_command(&args[1..]);
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
        "docintel-md\n\nUsage:\n  docintel-md analyze --input <file> [--output <dir>] [--endpoint <url>] [--key <key>] [--cloud global|21v] [--figure-mode inline|ignore|separate] [--split-pages <n>]\n  docintel-md split --input <file.pdf> [--output <dir>] [--pages-per-chunk <n>]\n  docintel-md segment --input <file.md> [--output <dir>] [--exam <code>]\n  docintel-md extract-json --manifest <manifest.json> [--output <dir>] [--from <n>] [--limit <n>]\n\nConfig can come from .env in the current directory, .env next to the exe, or environment variables:\n  DOCINTEL_ENDPOINT\n  DOCINTEL_KEY\n  DOCINTEL_CLOUD\n  DOCINTEL_API_VERSION\n  DOCINTEL_MODEL\n  DOCINTEL_FIGURE_MODE\n\nFigure modes:\n  inline    Keep figure OCR blocks in the main Markdown\n  ignore    Remove figure OCR blocks from the main Markdown\n  separate  Remove figure OCR blocks from the main Markdown and write them to *.figures.md\n\nLarge PDFs:\n  --split-pages <n> submits PDF chunks of n pages each, then writes a combined Markdown file\n  split only creates local PDF chunks, useful for testing before submitting\n\nQuestion pipeline:\n  segment splits a large Markdown export into one Markdown file per detected question and writes manifest.json\n  extract-json reads manifest.json and segments, then writes one structured JSON file per question\n\nDefaults:\n  --cloud global\n  --api-version 2024-11-30\n  --model prebuilt-layout\n  --figure-mode separate\n  --poll-seconds 1\n  --timeout-seconds 300\n  --pages-per-chunk 200\n  --exam SC-100\n  --from 1"
    );
}

fn split_command(args: &[String]) -> Result<(), String> {
    let flags = parse_flags(args)?;
    let input = flags
        .get("input")
        .map(PathBuf::from)
        .ok_or("missing --input <file.pdf>")?;
    if !input.exists() {
        return Err(format!("input not found: {}", input.display()));
    }
    if content_type_for(&input)? != "application/pdf" {
        return Err("split only supports PDF input".to_string());
    }

    let pages_per_chunk =
        parse_positive_u32(flags.get("pages-per-chunk"), "pages-per-chunk")?.unwrap_or(200);
    let output_dir = flags
        .get("output")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_split_dir(&input));
    let chunks = split_pdf(&input, &output_dir, pages_per_chunk)?;

    println!(
        "Split {} into {} chunk(s) under {}",
        input.display(),
        chunks.len(),
        output_dir.display()
    );
    for (index, chunk) in chunks.iter().enumerate() {
        println!(
            "  {:>3}. pages {}-{} of {} -> {}",
            index + 1,
            chunk.first_page,
            chunk.last_page,
            chunk.total_pages,
            chunk.path.display()
        );
    }
    Ok(())
}

fn segment_command(args: &[String]) -> Result<(), String> {
    let flags = parse_flags(args)?;
    let input = flags
        .get("input")
        .map(PathBuf::from)
        .ok_or("missing --input <file.md>")?;
    if !input.exists() {
        return Err(format!("input not found: {}", input.display()));
    }

    let exam = flags
        .get("exam")
        .cloned()
        .unwrap_or_else(|| "SC-100".to_string());
    let output_dir = flags
        .get("output")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_segment_dir(&input));
    segment_markdown(&input, &output_dir, &exam)
}

fn extract_json_command(args: &[String]) -> Result<(), String> {
    let flags = parse_flags(args)?;
    let manifest = flags
        .get("manifest")
        .or_else(|| flags.get("input"))
        .map(PathBuf::from)
        .ok_or("missing --manifest <manifest.json>")?;
    if !manifest.exists() {
        return Err(format!("manifest not found: {}", manifest.display()));
    }

    let from = parse_positive_u32(flags.get("from"), "from")?.unwrap_or(1) as usize;
    let limit = parse_positive_u32(flags.get("limit"), "limit")?.map(|value| value as usize);
    let output_dir = flags.get("output").map(PathBuf::from).unwrap_or_else(|| {
        manifest
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("questions")
    });

    extract_json_from_manifest(&manifest, &output_dir, from, limit)
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
        split_pages: parse_positive_u32(flags.get("split-pages"), "split-pages")?,
    })
}

fn parse_positive_u32(value: Option<&String>, name: &str) -> Result<Option<u32>, String> {
    let Some(value) = value else {
        return Ok(None);
    };
    let parsed = value
        .parse::<u32>()
        .map_err(|_| format!("--{name} must be a positive integer"))?;
    if parsed == 0 {
        return Err(format!("--{name} must be greater than 0"));
    }
    Ok(Some(parsed))
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
    if let Some(split_pages) = config.split_pages {
        return analyze_split_pdf(config, split_pages);
    }

    analyze_single(config)
}

fn analyze_split_pdf(config: Config, split_pages: u32) -> Result<(), String> {
    if content_type_for(&config.input)? != "application/pdf" {
        return Err("--split-pages only supports PDF input".to_string());
    }

    let output_dir = config
        .output
        .clone()
        .unwrap_or_else(|| default_output_dir(&config.input));
    fs::create_dir_all(&output_dir)
        .map_err(|e| format!("failed to create output dir {}: {e}", output_dir.display()))?;

    let chunks_dir = output_dir.join("_chunks");
    let chunks = split_pdf(&config.input, &chunks_dir, split_pages)?;
    println!(
        "Submitting {} PDF chunk(s) to Document Intelligence...",
        chunks.len()
    );

    let mut markdown_parts = Vec::new();
    for (index, chunk) in chunks.iter().enumerate() {
        let chunk_output_dir = output_dir.join(format!(
            "part-{index:03}-pages-{first:04}-{last:04}",
            index = index + 1,
            first = chunk.first_page,
            last = chunk.last_page
        ));
        let mut chunk_config = config.clone();
        chunk_config.input = chunk.path.clone();
        chunk_config.output = Some(chunk_output_dir.clone());
        chunk_config.split_pages = None;

        println!(
            "Chunk {}/{}: pages {}-{} of {}",
            index + 1,
            chunks.len(),
            chunk.first_page,
            chunk.last_page,
            chunk.total_pages
        );
        analyze_single(chunk_config)?;

        let chunk_stem = chunk
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("document");
        markdown_parts.push((
            chunk.first_page,
            chunk.last_page,
            chunk_output_dir.join(format!("{chunk_stem}.document-intelligence.md")),
        ));
    }

    write_combined_markdown(&config.input, &output_dir, &markdown_parts)?;
    Ok(())
}

fn analyze_single(config: Config) -> Result<(), String> {
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

fn split_pdf(
    input: &Path,
    output_dir: &Path,
    pages_per_chunk: u32,
) -> Result<Vec<PdfChunk>, String> {
    if pages_per_chunk == 0 {
        return Err("pages_per_chunk must be greater than 0".to_string());
    }

    fs::create_dir_all(output_dir)
        .map_err(|e| format!("failed to create output dir {}: {e}", output_dir.display()))?;

    let source = lopdf::Document::load(input)
        .map_err(|e| format!("failed to load PDF {}: {e}", input.display()))?;
    let page_ids = source.get_pages();
    let pages: Vec<u32> = page_ids.keys().copied().collect();
    if pages.is_empty() {
        return Err(format!("PDF has no pages: {}", input.display()));
    }

    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let total_pages = pages.len();
    let mut chunks = Vec::new();

    for (index, chunk_pages) in pages.chunks(pages_per_chunk as usize).enumerate() {
        let first_page = *chunk_pages.first().unwrap();
        let last_page = *chunk_pages.last().unwrap();
        let mut chunk_doc = source.clone();
        let selected_page_ids = chunk_pages
            .iter()
            .map(|page| {
                page_ids
                    .get(page)
                    .copied()
                    .ok_or_else(|| format!("page {page} not found in PDF page tree"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let catalog_id = chunk_doc
            .trailer
            .get(b"Root")
            .and_then(lopdf::Object::as_reference)
            .map_err(|e| format!("failed to find PDF catalog: {e}"))?;
        let pages_root_id = chunk_doc
            .get_object(catalog_id)
            .and_then(lopdf::Object::as_dict)
            .and_then(|dict| dict.get(b"Pages"))
            .and_then(lopdf::Object::as_reference)
            .map_err(|e| format!("failed to find PDF Pages root: {e}"))?;

        for page in &pages {
            if !chunk_pages.contains(page) {
                if let Some(page_id) = page_ids.get(page) {
                    chunk_doc.objects.remove(page_id);
                }
            }
        }

        for page_id in &selected_page_ids {
            let page = chunk_doc
                .objects
                .get_mut(page_id)
                .ok_or_else(|| format!("selected page object {:?} is missing", page_id))?;
            page.as_dict_mut()
                .map_err(|e| {
                    format!(
                        "selected page object {:?} is not a dictionary: {e}",
                        page_id
                    )
                })?
                .set("Parent", pages_root_id);
        }

        chunk_doc
            .objects
            .get_mut(&pages_root_id)
            .ok_or("PDF Pages root object is missing")?
            .as_dict_mut()
            .map_err(|e| format!("PDF Pages root is not a dictionary: {e}"))?
            .set(
                "Kids",
                selected_page_ids
                    .iter()
                    .copied()
                    .map(lopdf::Object::Reference)
                    .collect::<Vec<_>>(),
            );
        chunk_doc
            .objects
            .get_mut(&pages_root_id)
            .ok_or("PDF Pages root object is missing")?
            .as_dict_mut()
            .map_err(|e| format!("PDF Pages root is not a dictionary: {e}"))?
            .set("Count", selected_page_ids.len() as u32);

        chunk_doc.prune_objects();
        chunk_doc.renumber_objects();
        chunk_doc.compress();

        let chunk_path = output_dir.join(format!(
            "{stem}.part-{index:03}.pages-{first_page:04}-{last_page:04}.pdf",
            index = index + 1
        ));
        println!(
            "Writing chunk {}/{}: pages {}-{} of {} -> {}",
            index + 1,
            pages.chunks(pages_per_chunk as usize).len(),
            first_page,
            last_page,
            total_pages,
            chunk_path.display()
        );
        chunk_doc
            .save(&chunk_path)
            .map_err(|e| format!("failed to write PDF chunk {}: {e}", chunk_path.display()))?;

        chunks.push(PdfChunk {
            path: chunk_path,
            first_page,
            last_page,
            total_pages,
        });
    }

    Ok(chunks)
}

fn write_combined_markdown(
    input: &Path,
    output_dir: &Path,
    markdown_parts: &[(u32, u32, PathBuf)],
) -> Result<(), String> {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let combined_path = output_dir.join(format!("{stem}.document-intelligence.combined.md"));
    let mut combined = String::new();
    combined.push_str(&format!("# {}\n\n", stem));
    combined.push_str("由多个 PDF chunk 分别提交 Azure Document Intelligence 后合并生成。\n\n");

    for (index, (first_page, last_page, markdown_path)) in markdown_parts.iter().enumerate() {
        let content = fs::read_to_string(markdown_path)
            .map_err(|e| format!("failed to read {}: {e}", markdown_path.display()))?;
        combined.push_str(&format!(
            "## Part {}：PDF pages {}-{}\n\n",
            index + 1,
            first_page,
            last_page
        ));
        combined.push_str(content.trim_end());
        combined.push_str("\n\n");
    }

    fs::write(&combined_path, combined)
        .map_err(|e| format!("failed to write {}: {e}", combined_path.display()))?;
    println!("Wrote {}", combined_path.display());
    Ok(())
}

fn segment_markdown(input: &Path, output_dir: &Path, exam: &str) -> Result<(), String> {
    let content = fs::read_to_string(input)
        .map_err(|e| format!("failed to read {}: {e}", input.display()))?;
    let lines: Vec<&str> = content.lines().collect();
    let anchors = question_anchors(&lines);
    if anchors.is_empty() {
        return Err(format!(
            "no question headings found in {}; expected headings like '# Question #1'",
            input.display()
        ));
    }

    let segments_dir = output_dir.join("segments");
    if segments_dir.exists() {
        fs::remove_dir_all(&segments_dir).map_err(|e| {
            format!(
                "failed to remove stale segments dir {}: {e}",
                segments_dir.display()
            )
        })?;
    }
    fs::create_dir_all(&segments_dir).map_err(|e| {
        format!(
            "failed to create segments dir {}: {e}",
            segments_dir.display()
        )
    })?;

    let exam_slug = slugify(exam);
    let mut question_number_counts: HashMap<u32, usize> = HashMap::new();
    let mut items = Vec::new();
    for (index, (line_index, question_number)) in anchors.iter().enumerate() {
        let next_line_index = anchors
            .get(index + 1)
            .map(|(next, _)| *next)
            .unwrap_or(lines.len());
        let segment_text = lines[*line_index..next_line_index]
            .join("\n")
            .trim()
            .to_string()
            + "\n";
        let occurrence = question_number_counts
            .entry(*question_number)
            .and_modify(|count| *count += 1)
            .or_insert(1);
        let occurrence_value = *occurrence;
        let sequence_number = index + 1;
        let id = format!("{exam_slug}-{sequence_number:04}-q{question_number:04}");
        let file_name = format!("{id}.md");
        let segment_path = segments_dir.join(&file_name);
        fs::write(&segment_path, &segment_text)
            .map_err(|e| format!("failed to write {}: {e}", segment_path.display()))?;

        items.push(json!({
            "id": id,
            "exam": exam,
            "question_number": question_number,
            "question_number_occurrence": occurrence_value,
            "sequence_number": sequence_number,
            "segment_file": PathBuf::from("segments").join(&file_name).display().to_string(),
            "line_start": line_index + 1,
            "line_end": next_line_index,
            "content_hash": stable_hash_hex(&segment_text),
            "status": "segmented"
        }));
    }

    let manifest = json!({
        "exam": exam,
        "source_file": input.display().to_string(),
        "output_dir": output_dir.display().to_string(),
        "total_segments": items.len(),
        "items": items
    });
    let manifest_path = output_dir.join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap() + "\n",
    )
    .map_err(|e| format!("failed to write {}: {e}", manifest_path.display()))?;

    println!(
        "Segmented {} question(s) from {} into {}",
        anchors.len(),
        input.display(),
        segments_dir.display()
    );
    println!("Wrote {}", manifest_path.display());
    Ok(())
}

fn extract_json_from_manifest(
    manifest_path: &Path,
    output_dir: &Path,
    from: usize,
    limit: Option<usize>,
) -> Result<(), String> {
    fs::create_dir_all(output_dir)
        .map_err(|e| format!("failed to create output dir {}: {e}", output_dir.display()))?;

    let manifest_text = fs::read_to_string(manifest_path)
        .map_err(|e| format!("failed to read {}: {e}", manifest_path.display()))?;
    let manifest: Value = serde_json::from_str(&manifest_text)
        .map_err(|e| format!("failed to parse manifest JSON: {e}"))?;
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let exam = manifest
        .get("exam")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let items = manifest
        .get("items")
        .and_then(Value::as_array)
        .ok_or("manifest missing items array")?;

    let mut written = 0usize;
    let mut needs_review = 0usize;
    for item in items {
        let sequence_number = value_as_usize(item, "sequence_number")?;
        if sequence_number < from {
            continue;
        }
        if let Some(limit) = limit {
            if written >= limit {
                break;
            }
        }

        let segment_file = item
            .get("segment_file")
            .and_then(Value::as_str)
            .ok_or("manifest item missing segment_file")?;
        let segment_path = manifest_dir.join(segment_file);
        let segment_text = fs::read_to_string(&segment_path)
            .map_err(|e| format!("failed to read segment {}: {e}", segment_path.display()))?;
        let parsed = parse_question_segment(&segment_text);

        let mut warnings = parsed.warnings;
        let question_number_occurrence = value_as_usize(item, "question_number_occurrence")?;
        if question_number_occurrence > 1 {
            warnings.push("source_question_number_is_not_unique".to_string());
        }
        if segment_text.chars().count() > 12_000 {
            warnings.push("segment_is_large_for_single_ai_pass".to_string());
        }
        warnings.sort();
        warnings.dedup();

        let status = if warnings.iter().any(|warning| {
            matches!(
                warning.as_str(),
                "no_options_detected"
                    | "no_correct_answer_detected"
                    | "correct_answer_not_found_in_options"
                    | "non_standard_question_type"
            )
        }) {
            needs_review += 1;
            "needs_review"
        } else {
            "parsed"
        };

        let id = item.get("id").and_then(Value::as_str).unwrap_or("question");
        let question_json = json!({
            "schema_version": 1,
            "id": id,
            "exam": exam,
            "sequence_number": sequence_number,
            "question_number_from_source": value_as_usize(item, "question_number")?,
            "question_number_occurrence": question_number_occurrence,
            "answer_type": parsed.answer_type,
            "topic": parsed.topic,
            "question": {
                "original": parsed.question,
                "zh_cn": null
            },
            "options": parsed.options.iter().map(|(key, value)| json!({
                "key": key,
                "original": value,
                "zh_cn": null
            })).collect::<Vec<_>>(),
            "correct_answer": parsed.correct_answer,
            "explanation": {
                "original": parsed.explanation,
                "zh_cn": null,
                "summary": null
            },
            "discussion": {
                "original": parsed.discussion,
                "zh_cn": null
            },
            "source": {
                "manifest": manifest_path.display().to_string(),
                "segment_file": segment_file,
                "line_start": value_as_usize(item, "line_start")?,
                "line_end": value_as_usize(item, "line_end")?,
                "content_hash": item.get("content_hash").and_then(Value::as_str).unwrap_or_default()
            },
            "confidence": {
                "mechanical_parse": parsed.confidence
            },
            "status": status,
            "warnings": warnings,
            "raw_segment": segment_text
        });

        let output_path = output_dir.join(format!("{id}.json"));
        fs::write(
            &output_path,
            serde_json::to_string_pretty(&question_json).unwrap() + "\n",
        )
        .map_err(|e| format!("failed to write {}: {e}", output_path.display()))?;
        written += 1;
    }

    println!(
        "Extracted {} question JSON file(s) into {} ({} need review)",
        written,
        output_dir.display(),
        needs_review
    );
    Ok(())
}

#[derive(Debug)]
struct ParsedQuestion {
    topic: Option<String>,
    question: String,
    options: Vec<(String, String)>,
    correct_answer: Vec<String>,
    answer_type: String,
    explanation: String,
    discussion: String,
    confidence: f64,
    warnings: Vec<String>,
}

fn parse_question_segment(segment: &str) -> ParsedQuestion {
    let lines: Vec<&str> = segment.lines().collect();
    let mut warnings = Vec::new();
    let comments_index = lines.iter().position(|line| is_comments_heading(line));
    let main_end = comments_index.unwrap_or(lines.len());
    let main_lines = &lines[..main_end];
    let discussion = comments_index
        .map(|index| lines[index..].join("\n").trim().to_string())
        .unwrap_or_default();
    let topic = main_lines
        .iter()
        .map(|line| line.trim().trim_start_matches('#').trim())
        .find(|line| line.starts_with("Topic "))
        .map(ToString::to_string);

    let answer_index = main_lines
        .iter()
        .position(|line| line.trim().starts_with("Correct Answer:"));
    let correct_answer = answer_index
        .and_then(|index| main_lines.get(index))
        .map(|line| parse_correct_answer(line))
        .unwrap_or_default();
    if correct_answer.is_empty() {
        warnings.push("no_correct_answer_detected".to_string());
    }

    let question_area_end = answer_index.unwrap_or(main_lines.len());
    let question_area = &main_lines[..question_area_end];
    let (question, options) = parse_question_and_options(question_area);
    if options.is_empty() {
        warnings.push("no_options_detected".to_string());
    }

    let option_keys: Vec<&str> = options.iter().map(|(key, _)| key.as_str()).collect();
    if !correct_answer.is_empty()
        && correct_answer
            .iter()
            .any(|answer| !option_keys.contains(&answer.as_str()))
    {
        warnings.push("correct_answer_not_found_in_options".to_string());
    }

    let non_standard = segment.contains("HOTSPOT")
        || segment.contains("Hot Area")
        || segment.contains("DRAG DROP")
        || segment.contains("Case Study")
        || options.is_empty();
    let answer_type = if non_standard {
        warnings.push("non_standard_question_type".to_string());
        "needs_review".to_string()
    } else if correct_answer.len() > 1 {
        "multiple_choice".to_string()
    } else {
        "single_choice".to_string()
    };

    let answer_tail = answer_index
        .map(|index| main_lines[index + 1..].join("\n").trim().to_string())
        .unwrap_or_default();
    let explanation = build_explanation(&answer_tail, &discussion);
    let confidence = mechanical_confidence(&warnings, options.len(), correct_answer.len());

    ParsedQuestion {
        topic,
        question,
        options,
        correct_answer,
        answer_type,
        explanation,
        discussion,
        confidence,
        warnings,
    }
}

fn parse_question_and_options(lines: &[&str]) -> (String, Vec<(String, String)>) {
    let mut question_lines = Vec::new();
    let mut options = Vec::new();
    let mut current_option: Option<(String, Vec<String>)> = None;

    for line in lines {
        if let Some((key, value)) = parse_option_start(line) {
            if let Some((previous_key, previous_lines)) = current_option.take() {
                options.push((previous_key, cleanup_text(&previous_lines.join("\n"))));
            }
            current_option = Some((key, vec![value]));
            continue;
        }

        if let Some((_, option_lines)) = current_option.as_mut() {
            option_lines.push((*line).to_string());
        } else if should_keep_question_line(line) {
            question_lines.push((*line).to_string());
        }
    }

    if let Some((previous_key, previous_lines)) = current_option.take() {
        options.push((previous_key, cleanup_text(&previous_lines.join("\n"))));
    }

    (cleanup_text(&question_lines.join("\n")), options)
}

fn parse_option_start(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    let mut chars = trimmed.chars();
    let key = chars.next()?;
    let marker = chars.next()?;
    if !key.is_ascii_uppercase() || !matches!(marker, '.' | ')') {
        return None;
    }
    let rest: String = chars.collect();
    if rest.trim().is_empty() {
        return None;
    }
    Some((key.to_string(), rest.trim().to_string()))
}

fn parse_correct_answer(line: &str) -> Vec<String> {
    let Some((_, answer)) = line.split_once(':') else {
        return Vec::new();
    };
    answer
        .chars()
        .filter(|ch| ch.is_ascii_uppercase())
        .map(|ch| ch.to_string())
        .collect()
}

fn build_explanation(answer_tail: &str, discussion: &str) -> String {
    let answer_tail = cleanup_text(answer_tail);
    let discussion_explanation = best_discussion_explanation(discussion);
    if !discussion_explanation.is_empty()
        && (answer_tail.is_empty()
            || answer_tail.contains("Community vote distribution")
            || answer_tail.chars().count() < 120)
    {
        return discussion_explanation;
    }
    if !discussion_explanation.is_empty() {
        return format!("{}\n\n---\n\n{}", answer_tail, discussion_explanation);
    }
    answer_tail
}

fn best_discussion_explanation(discussion: &str) -> String {
    let mut best_score = 0usize;
    let mut best_block = String::new();
    for block in discussion.split("upvoted ") {
        let cleaned = cleanup_text(block);
        let char_count = cleaned.chars().count();
        if char_count < 120 {
            continue;
        }
        let lower = cleaned.to_ascii_lowercase();
        let mut score = 0usize;
        for marker in [
            "selected answer",
            "explanation",
            " is the answer",
            " correct",
            "because",
            "supports",
            "requires",
            "gives you",
            "capability",
            "https://",
        ] {
            if lower.contains(marker) {
                score += 1;
            }
        }
        score += (char_count / 400).min(3);
        if score > best_score {
            best_score = score;
            best_block = cleaned;
        }
    }
    best_block
}

fn is_comments_heading(line: &str) -> bool {
    line.trim().trim_start_matches('#').trim() == "Comments"
}

fn should_keep_question_line(line: &str) -> bool {
    let trimmed = line.trim().trim_start_matches('#').trim();
    !trimmed.is_empty()
        && !trimmed.starts_with("Question #")
        && !trimmed.starts_with("Topic ")
        && trimmed != "EXAMTOPICS"
        && !trimmed.starts_with("<!--")
}

fn cleanup_text(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn mechanical_confidence(warnings: &[String], option_count: usize, answer_count: usize) -> f64 {
    let mut confidence = 0.95;
    confidence -= warnings.len() as f64 * 0.12;
    if option_count < 2 {
        confidence -= 0.2;
    }
    if answer_count == 0 {
        confidence -= 0.2;
    }
    confidence.clamp(0.05, 0.95)
}

fn value_as_usize(value: &Value, key: &str) -> Result<usize, String> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .ok_or_else(|| format!("manifest item missing numeric {key}"))
}

fn question_anchors(lines: &[&str]) -> Vec<(usize, u32)> {
    lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| parse_question_heading(line).map(|number| (index, number)))
        .collect()
}

fn parse_question_heading(line: &str) -> Option<u32> {
    let normalized = line.trim().trim_start_matches('#').trim();
    let rest = normalized.strip_prefix("Question #")?;
    let digits: String = rest.chars().take_while(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "questions".to_string()
    } else {
        slug
    }
}

fn stable_hash_hex(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
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
        let response_text = read_response_text(response)?;
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

fn read_response_text(response: ureq::Response) -> Result<String, String> {
    let mut response_text = String::new();
    response
        .into_reader()
        .read_to_string(&mut response_text)
        .map_err(|e| format!("failed to read service response: {e}"))?;
    Ok(response_text)
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

fn default_split_dir(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    PathBuf::from("output").join("pdf-chunks").join(stem)
}

fn default_segment_dir(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    PathBuf::from("output").join("question-pipeline").join(stem)
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
            let text = read_response_text(response)
                .unwrap_or_else(|_| "<failed to read response body>".to_string());
            format!("service returned HTTP {code}: {text}")
        }
        ureq::Error::Transport(transport) => format!("transport error: {transport}"),
    }
}
