# Skill 整理规划：从 PDF/Prompt 到结构化题库的统一工具设计

## 一、现有两条工作流总结

本项目有两条独立的题库生产线，最终都输出到同一套 SQLite schema 供 TauriExam 消费。

### 工作流 A：PDF 视觉读题（sc100-vision-md）

**用途**：把一份图文混排的考试题库 PDF（含题目、答案、社区讨论）整理成结构化题库。

**完整链路**：

```
用户输入：一份 PDF 文件（如 SC-100 题目+答案+讨论.pdf，~793页）
   │
   ▼
┌─ Step 0：进度探测 ──────────────────────────────────────────┐
│  detect_next_batch.py                                       │
│  - 扫描 output/vision-md/ 下已有 Markdown                   │
│  - 解析最后处理到的页码和题号                                 │
│  - 读取 PDF 总页数，判断是否全部完成                          │
│  - 输出 JSON：done/next_page_range/carryover/建议输出路径    │
│  - 如果 done=true → 整个流程结束                             │
└──────────────────────────────────────────────────────────────┘
   │ (done=false)
   ▼
┌─ Step 1：PDF 页面渲染 ──────────────────────────────────────┐
│  render_pdf_pages.py                                        │
│  - 输入：PDF路径 + 页范围（默认30页窗口）                     │
│  - 用 PyMuPDF 将每页渲染为 PNG（1.6x缩放）                   │
│  - 输出：output/vision-pages/sc-100-pages-XXX-YYY/*.png     │
└──────────────────────────────────────────────────────────────┘
   │
   ▼
┌─ Step 2：辅助文本提取 ──────────────────────────────────────┐
│  extract_page_text.py                                       │
│  - 输入：PDF路径 + 页范围                                    │
│  - 用 PyMuPDF 提取每页文本层                                 │
│  - 输出：page-text-XXX-YYY.txt（按页分段）                   │
│  - 注意：此文本仅作辅助，不能替代视觉审阅                     │
└──────────────────────────────────────────────────────────────┘
   │
   ▼
┌─ Step 3：AI 视觉读题 + 判答（核心 AI 环节）────────────────┐
│  由 Copilot / LLM 执行，SKILL.md 作为 system prompt         │
│  - 逐页查看 PNG 图片，识别 Question 边界                     │
│  - 对照辅助文本抄录题干和选项                                 │
│  - HOTSPOT/DRAG DROP/表格题按视觉内容理解答案区              │
│  - 保留源答案 + 给出 AI 推荐答案 + 中文解析                  │
│  - 处理跨页/跨批次的 carryover 衔接                          │
│  - 每批严格 10 题                                            │
└──────────────────────────────────────────────────────────────┘
   │
   ▼
┌─ Step 4：输出结构化 Markdown ──────────────────────────────┐
│  output/vision-md/sc-100-pages-XXX-YYY/                     │
│    sc-100-questions-NNN-MMM.md                               │
│  每题包含：                                                   │
│    Source pages / Topic / Type / Status                       │
│    Question / Options 或 Answer Area                         │
│    Source Answer / My Recommended Answer                     │
│    我的判断（中文）/ Reasoning / Notes                        │
└──────────────────────────────────────────────────────────────┘
   │
   ▼
┌─ Step 5：Markdown → SQLite 入库 ───────────────────────────┐
│  scripts/import_vision_md_to_sqlite.py                      │
│  - 解析所有 sc-100-questions-*.md 文件                       │
│  - 提取结构化字段 → 写入 4 张表                              │
│    markdown_batches / questions / options / answer_areas     │
│  - 输出：output/vision-db/sc-100.sqlite                     │
└──────────────────────────────────────────────────────────────┘
   │
   ▼
最终产物：SQLite 题库 → TauriExam 桌面应用读取使用
```

**自动化外壳**：`continue-sc100.ps1` 循环最多 35 轮，每轮先调 `detect_next_batch.py` 判断进度，再调 `copilot CLI` 触发 SKILL 执行 Step 1-4。用户只需运行一次脚本即可从头处理到尾。

**关键特点**：
- 三层架构：外循环调度（ps1）+ 状态探测（Python）+ AI 大脑（SKILL.md）
- 无状态续接：每轮从文件系统反推进度，可中断可恢复
- 视觉优先：不盲信 OCR，强制看 PNG 图片判题
- 答案双轨：保留源答案 + AI 推荐答案 + 中文解析
- Carryover 机制：自动处理跨页/跨批次的题目衔接

---

### 工作流 B：AI 原创生成题目（exam-question-gen）

**用途**：基于微软官方认证大纲，AI 从零原创生成模拟题。

**完整链路**：

```
用户输入：一句提示词（如"随机生成 AI-900 的题目"）
   │
   ▼
┌─ Step 1：确定考试范围 ─────────────────────────────────────┐
│  由 AI 根据用户指定的考试代号确认：                           │
│  - 考试全称和 Skills Measured 各领域及权重                    │
│  - 当前有效的产品/服务名称                                    │
│  - 支持：AI-900, AZ-900, AZ-104, AZ-305, SC-100,           │
│          SC-200, DP-900, DP-203 等                           │
└──────────────────────────────────────────────────────────────┘
   │
   ▼
┌─ Step 2：批次续接检测 ─────────────────────────────────────┐
│  检查 output/exam-gen/<exam-code>/ 下已有文件                │
│  - 自动编号续接（如已有 001-010，下一批 011-020）             │
│  - 避免与已有题目重复（检查题干关键词）                       │
└──────────────────────────────────────────────────────────────┘
   │
   ▼
┌─ Step 3：AI 生成题目（核心 AI 环节）───────────────────────┐
│  由 Copilot / LLM 执行，SKILL.md 作为 system prompt         │
│  每批生成 10 题，遵循：                                       │
│  - 各题分布在不同知识域，比例对齐官方权重                     │
│  - 题型多样：single_choice(60%) + hotspot/drag_drop/yes_no   │
│  - 难度分级：easy/medium(默认)/hard                           │
│  - 场景化题干（公司名、需求、约束条件）                       │
│  - 合理干扰项（常见误解，不是垃圾选项）                       │
│  - 每题含正确答案 + 中文解析 + Key Concept                   │
└──────────────────────────────────────────────────────────────┘
   │
   ▼
┌─ Step 4：输出结构化 Markdown ──────────────────────────────┐
│  output/exam-gen/<exam-code>/questions-NNN-MMM.md            │
│  每题包含：                                                   │
│    Exam / Topic / Type / Difficulty                           │
│    Question / Options 或 Answer Area 或 Statements           │
│    Correct Answer / 解析（中文）/ Key Concept                │
└──────────────────────────────────────────────────────────────┘
   │
   ▼
┌─ Step 5：Markdown → SQLite 入库 ───────────────────────────┐
│  scripts/import_exam_gen_to_sqlite.py                        │
│  - 解析所有 questions-*.md 文件                               │
│  - 映射到同一套 TauriExam schema                              │
│    questions / options / answer_areas                         │
│  - 输出：question-banks/<EXAM>.sqlite                        │
└──────────────────────────────────────────────────────────────┘
   │
   ▼
最终产物：SQLite 题库 → TauriExam 桌面应用读取使用
```

**关键特点**：
- 无外循环脚本：目前靠人工说"再来一批"或"继续出题"
- 轻量状态检测：只查目录已有文件编号
- 纯 AI 生成：不需要 PDF/图片处理
- 同一 SQLite schema：入库后与 PDF 题库格式兼容

---

## 二、两条工作流的对比

| 维度 | PDF 视觉读题 (A) | AI 原创生成 (B) |
|---|---|---|
| **输入** | PDF 文件 | 考试代号 + 提示词 |
| **AI 角色** | 视觉审阅 + 判答 + 格式化 | 原创出题 + 解析 |
| **需要视觉能力** | 是（必须看 PNG 图片） | 否 |
| **需要 PDF 处理** | 是（渲染 + 文本提取） | 否 |
| **进度管理** | detect_next_batch.py（复杂） | 简单目录扫描 |
| **自动化程度** | 高（continue-sc100.ps1 全自动） | 低（人工触发每批） |
| **Carryover** | 必须处理跨页题目衔接 | 不需要 |
| **答案来源** | 双轨（源答案 + AI 推荐） | 单轨（AI 生成即正确） |
| **Markdown 中间层** | output/vision-md/ | output/exam-gen/ |
| **入库脚本** | import_vision_md_to_sqlite.py | import_exam_gen_to_sqlite.py |
| **SQLite 输出** | output/vision-db/sc-100.sqlite | question-banks/<EXAM>.sqlite |

**共同点**：
- 都以 Markdown 作为质量闸口（先 MD 再 SQLite）
- 都用 SKILL.md 驱动 AI 行为
- 最终都输出兼容的 SQLite schema
- 都被 TauriExam 桌面应用消费

---

## 三、统一工具设计目标

**核心愿景**：用户只需提供一个 PDF 文件或一句提示词，工具自动完成从输入到结构化题库的全流程。

```
┌─────────────────────────────────────────────────────────────┐
│                      ExamForge CLI / GUI                     │
│                                                              │
│  用户输入 A：一份 PDF                                         │
│    examforge from-pdf SC-100.pdf                             │
│    → 自动渲染→视觉读题→写MD→入库→产出 SQLite               │
│                                                              │
│  用户输入 B：一句提示词                                       │
│    examforge generate AI-900 --count 50                      │
│    → 自动出题→写MD→入库→产出 SQLite                         │
│                                                              │
│  用户输入 C：已有 Markdown                                    │
│    examforge import output/vision-md/ --exam SC-100          │
│    → 直接入库→产出 SQLite                                    │
└─────────────────────────────────────────────────────────────┘
```

---

## 四、统一工具架构设计

### 4.1 整体架构

```
ExamForge
├── CLI 入口（examforge 命令）
│   ├── from-pdf     ← 工作流 A
│   ├── generate     ← 工作流 B
│   ├── import       ← 仅入库（跳过 AI）
│   ├── status       ← 查看进度
│   └── serve        ← 可选：本地 Web UI
│
├── 核心模块
│   ├── pdf-engine        PDF 渲染 + 文本提取
│   ├── progress-tracker  进度探测与续接管理
│   ├── ai-driver         AI 调度（Copilot CLI / OpenAI API）
│   ├── prompt-manager    SKILL 提示词组装与注入
│   ├── md-parser         Markdown 解析与校验
│   ├── sqlite-writer     统一 SQLite 入库
│   └── config            配置管理（API Key / 模型 / 路径）
│
└── 数据流向
    输入(PDF/Prompt) → AI处理 → Markdown → SQLite → TauriExam
```

### 4.2 模块详细设计

#### 模块 1：PDF Engine（PDF 处理引擎）

**职责**：替代现有 `render_pdf_pages.py` + `extract_page_text.py`

```rust
// pdf_engine.rs — 封装 pdfium-render
pub struct PdfEngine { /* pdfium binding */ }

impl PdfEngine {
    pub fn open(path: &Path) -> Result<Self>;
    pub fn page_count(&self) -> usize;
    pub fn render_pages(&self, range: PageRange, scale: f32, out_dir: &Path) -> Result<Vec<PathBuf>>;
    pub fn extract_text(&self, range: PageRange) -> Result<String>;
}

实现要点：
  - 直接复用 TauriExam 已验证的 pdfium-render + image png 组合
  - build.rs 复制 pdfium.dll/so 到 target/ 旁边
  - 默认 scale 1.6，可配置
  - 渲染输出 PNG，提取输出分页文本
```

#### 模块 2：Progress Tracker（进度管理器）

**职责**：替代 `detect_next_batch.py` + 工作流 B 的简单目录扫描

```rust
// progress.rs
pub struct ProgressState {
    pub done: bool,
    pub total_pages: Option<usize>,
    pub completed_pages: usize,
    pub completed_questions: usize,
    pub next_batch: Option<BatchPlan>,
    pub carryover: Option<String>,
}

pub struct BatchPlan {
    pub page_range: (usize, usize),
    pub expected_question_start: usize,
    pub output_dir: PathBuf,
}

pub fn detect(mode: Mode, output_root: &Path, pdf_pages: Option<usize>) -> Result<ProgressState>;

设计要点：
  - 无状态：每次从文件系统反推进度
  - 可中断可恢复：crash 后重启自动续接
  - 两种模式共用同一个进度概念：
    PDF 模式：按页码追踪
    生成模式：按题号追踪
```

#### 模块 3：AI Driver（AI 调度器）

**职责**：替代 `continue-sc100.ps1` 循环 + Copilot CLI 调用

```rust
// ai/mod.rs
pub trait AiDriver {
    fn complete(&self, prompt: &Prompt) -> Result<String>;
    fn complete_with_images(&self, prompt: &Prompt, images: &[PathBuf]) -> Result<String>;
}

// ai/copilot.rs — 调 copilot CLI
pub struct CopilotDriver { model: String, effort: String }
impl AiDriver for CopilotDriver {
    // 内部用 std::process::Command 调 copilot CLI
    // SKILL.md 由 Copilot 自动加载
}

// ai/openai.rs — OpenAI 兼容 API
pub struct OpenAiDriver { endpoint: String, api_key: String, model: String }
impl AiDriver for OpenAiDriver {
    // 内部用 ureq POST /chat/completions
    // SKILL.md 内容作为 system prompt 注入
    // PNG 图片 base64 编码后放入 vision message
}
```

设计要点：
  - trait 抽象：调用方不关心后端是 Copilot 还是 API
  - 统一重试逻辑（网络错误 / 超时 / 格式不合规）
  - 输出校验：AI 返回的文本必须通过 Markdown 格式检查
  - 流式输出：支持 SSE 实时显示 AI 正在生成的内容
  - 单次 token 控制：避免超长输出导致截断
```

#### 模块 4：Prompt Manager（提示词管理器）

**职责**：将 SKILL.md 的内容动态组装为 AI 可用的 prompt

```
PDF 模式 prompt 组装：
  system = SKILL.md 核心原则 + 质量规则 + 模板格式
  user   = "处理页范围 {from}-{to}"
         + "上一批 carryover: {carryover_text}"
         + "期望题号从 {next_q} 开始"
         + [附带：PNG 图片 + 辅助文本]

生成模式 prompt 组装：
  system = SKILL.md 核心原则 + 质量规则 + 模板格式
  user   = "为 {exam_code} 生成 10 道模拟题"
         + "已有题号到 {last_q}，从 {next_q} 开始"
         + "难度: {difficulty}"
         + "可选主题偏好: {topic_filter}"

Copilot CLI 模式：
  - 不需要手动注入 SKILL.md（由 Copilot 自动加载）
  - 只需组装 user prompt 部分

OpenAI API 模式：
  - 需要把 SKILL.md 全文读取并拼接到 system message
  - PNG 图片需 base64 编码后放入 vision message
```

#### 模块 5：Markdown Parser & Validator（MD 解析与校验）

**职责**：统一的 Markdown 解析，替代两个独立的入库脚本中的解析逻辑

```
功能：
  parse(md_text, mode) → list[Question]
    mode = "vision" | "exam-gen"
  
  validate(questions) → list[ValidationError]
    检查：
    - 每批恰好 10 题（或最后一批允许少于 10）
    - 每题有完整的必要字段
    - 选项/答案区格式正确
    - 无重复题号

共用数据模型：
  Question {
    id, exam, sequence_number
    topic, question_type, difficulty, status
    question_text
    options: Option[]          // A/B/C/D 选项
    answer_area: AnswerRow[]   // HOTSPOT/DRAG DROP 表格
    source_answer              // 源答案（PDF 模式）
    recommended_answer         // 推荐答案 / 正确答案
    chinese_explanation        // 中文解析
    key_concept               // 核心知识点（生成模式）
    reasoning                 // 推理过程（PDF 模式）
    source_pages              // PDF 页码（PDF 模式）
    notes
    raw_markdown              // 原始 Markdown 块
  }
```

#### 模块 6：SQLite Writer（统一入库器）

**职责**：替代两个独立的 `import_*_to_sqlite.py` 脚本

```rust
// sqlite_writer.rs
pub struct WriteOptions {
    pub reset: bool,
    pub exam: String,
    pub source: Mode,       // Vision | ExamGen
    pub pdf_path: Option<PathBuf>,
}

pub fn write(questions: &[Question], db_path: &Path, options: &WriteOptions) -> Result<usize>;

Schema（保持现有 TauriExam 兼容）：
  markdown_batches  ← 仅 vision 模式有
  questions         ← 统一主表
  options           ← 选择题选项
  answer_areas      ← HOTSPOT/DRAG DROP

设计要点：
  - rusqlite bundled，无需系统安装 SQLite
  - 两种来源写入同一套 schema
  - 幂等：重复导入不会产生重复数据（INSERT OR REPLACE）
  - 增量：可追加新批次而不重建整库
```

---

## 五、CLI 命令设计

### 5.1 命令一览

```bash
examforge from-pdf <pdf-path> [options]     # PDF → 全自动处理到 SQLite
examforge generate <exam-code> [options]    # AI 生成题库
examforge import <md-dir> [options]         # 仅 Markdown → SQLite
examforge status [options]                  # 查看处理进度
examforge config [options]                  # 管理配置（API Key 等）
```

### 5.2 from-pdf 子命令（工作流 A 一键化）

```bash
examforge from-pdf SC-100.pdf
  --exam SC-100                        # 考试代号（默认从文件名推断）
  --output ./output                    # 输出根目录
  --db ./question-banks/SC-100.sqlite  # SQLite 输出路径
  --batch-size 30                      # 每批渲染页数
  --questions-per-batch 10             # 每批 Markdown 题数
  --ai copilot                         # AI 后端：copilot | openai
  --model gpt-4.1                      # 模型
  --api-key <key>                      # OpenAI 模式的 API Key
  --endpoint <url>                     # 自定义 API 端点
  --resume                             # 断点续接（默认行为）
  --from-page 100                      # 从指定页开始
  --dry-run                            # 只打印计划，不执行
  --no-import                          # 只生成 Markdown，不入库
  --verbose                            # 详细日志
```

**执行流程**：
```
1. 检测 PDF 有效性和总页数
2. 扫描已有输出，计算续接点
3. 循环：
   a. 渲染当前批次页面 → PNG
   b. 提取辅助文本 → TXT
   c. 组装 prompt（含图片 + 文本 + carryover）
   d. 调用 AI → 获取 Markdown
   e. 校验 Markdown 格式
   f. 写入 Markdown 文件
   g. 更新进度
4. 全部完成后自动入库 → SQLite
5. 打印统计报告
```

### 5.3 generate 子命令（工作流 B 一键化）

```bash
examforge generate AI-900
  --count 50                           # 总题数（默认 10）
  --difficulty medium                  # easy | medium | hard
  --topic "Azure AI Services"          # 主题过滤（可选）
  --types single_choice,hotspot        # 题型过滤（可选）
  --output ./output/exam-gen/AI-900    # Markdown 输出目录
  --db ./question-banks/AI-900.sqlite  # SQLite 输出路径
  --ai copilot                         # AI 后端
  --model gpt-4.1                      # 模型
  --resume                             # 续接已有题目
  --no-import                          # 只生成 Markdown
```

**执行流程**：
```
1. 确认考试代号和大纲
2. 扫描已有输出，确定续接题号
3. 循环（每轮 10 题）：
   a. 组装 prompt（含大纲 + 已有题干关键词去重）
   b. 调用 AI → 获取 Markdown
   c. 校验 Markdown 格式
   d. 写入 Markdown 文件
   e. 更新进度
4. 全部完成后自动入库 → SQLite
5. 打印统计报告
```

### 5.4 import 子命令（仅入库）

```bash
examforge import output/vision-md/
  --exam SC-100
  --mode vision                        # vision | exam-gen（自动检测）
  --db ./question-banks/SC-100.sqlite
  --reset                              # 重建数据库
```

### 5.5 status 子命令

```bash
examforge status
  --input SC-100.pdf                   # 查看指定 PDF 的处理进度
  --exam AI-900                        # 查看指定考试的生成进度

# 输出示例：
# SC-100 PDF Processing:
#   PDF: SC-100.pdf (793 pages)
#   Processed: 774/793 pages (97.6%)
#   Questions: 310 extracted
#   Last batch: sc-100-questions-301-310.md
#   Next: pages 775-804
#   Status: IN PROGRESS
```

---

## 六、AI 后端对接方案

### 6.1 方案 A：Copilot CLI（推荐初期）

```
优点：
  - 最小改造量：保持现有 SKILL.md + continue-sc100.ps1 的机制
  - SKILL.md 自动加载
  - 视觉能力内置
  - 只需 GitHub Copilot 订阅
  
缺点：
  - 依赖 copilot CLI 工具安装
  - 输出捕获和错误处理不如 API 精细
  - 不能精确控制 token / temperature

调用方式：
  copilot --model <model> --effort <effort> -p "<prompt>" --allow-all

适合场景：
  - 个人使用
  - 已有 Copilot 订阅
  - 快速上手
```

### 6.2 方案 B：OpenAI 兼容 API

```
优点：
  - 完全可控（模型、参数、重试、流式输出）
  - 兼容 OpenAI / Azure OpenAI / 本地 Ollama / vLLM / 任意兼容端点
  - 不依赖 Copilot CLI
  - 可以精确传图片（base64 in vision message）

缺点：
  - 需要自己注入 SKILL.md 内容作为 system prompt
  - 需要自己处理图片的 base64 编码和传输
  - API 按量计费

调用方式：
  POST <endpoint>/chat/completions
  {
    "model": "<model>",
    "messages": [
      {"role": "system", "content": "<SKILL.md 内容>"},
      {"role": "user", "content": [
        {"type": "text", "text": "<prompt>"},
        {"type": "image_url", "url": {"url": "data:image/png;base64,..."}}
      ]}
    ],
    "max_tokens": 8192,
    "temperature": 0.3
  }

配置文件 (~/.examforge/config.toml)：
  [ai]
  backend = "openai"          # "copilot" | "openai"
  model = "gpt-4.1"
  endpoint = "https://api.openai.com/v1"
  api_key = "sk-..."          # 或环境变量 EXAMFORGE_API_KEY
  max_retries = 3
  temperature = 0.3

适合场景：
  - 团队使用 / CI/CD
  - 需要精确控制
  - 使用自建模型
```

---

## 七、技术选型建议

### 7.1 CLI 工具本体

```
选定方案：Rust CLI（单二进制，零运行时依赖）

理由：
  - 与 TauriExam 同栈，可共享 crate / 数据模型
  - pdfium-render、rusqlite、ureq、serde 已在 TauriExam 验证
  - 编译后单文件 .exe，无需安装 Python/Node 运行环境
  - Release profile 可压缩到 5-10 MB
  - 未来可直接编译为 TauriExam 的 library crate，嵌入桌面应用
  - 交叉编译 linux/mac/windows 开箱即用
```

### 7.2 推荐技术栈

```
核心依赖（全部已在本项目或 TauriExam 验证过）：

  CLI 框架    ：clap 4（derive 模式，自动生成帮助和补全）
  PDF 渲染    ：pdfium-render 0.9 + 捆绑 pdfium.dll/so
                TauriExam 已验证这条路径，build.rs 复制 pdfium 二进制
  PDF 文本层  ：lopdf 0.34（已在根 Cargo.toml 使用）或 pdfium-render 自带文本提取
  图片编码    ：image 0.25（PNG 读写）+ base64 0.22（vision API 传图）
  SQLite      ：rusqlite 0.32 bundled（与 TauriExam 同版本）
  HTTP/AI API ：ureq 2（同步，轻量）或 reqwest（如需异步流式）
  JSON        ：serde + serde_json
  正则解析    ：regex crate（替代 Python re 模块）
  配置管理    ：toml crate（读写 TOML 配置文件）
  进度展示    ：indicatif（终端进度条）+ console（彩色输出）
  日志        ：tracing + tracing-subscriber（结构化日志）
  进程调用    ：std::process::Command（调 copilot CLI）
  SHA 哈希    ：sha2 0.10（文件去重，已在 TauriExam 使用）

可选增强：
  异步 AI 流  ：reqwest + tokio（SSE 流式输出实时显示）
  模板引擎    ：minijinja（组装 prompt 模板）
  测试        ：内置 #[test] + assert_cmd（CLI 集成测试）
```

---

## 八、项目结构建议

```
examforge/
├── Cargo.toml                  # workspace root
├── README.md
├── resources/
│   └── pdfium.dll              # 捆绑 PDFium 二进制（与 TauriExam 共用）
│
├── crates/
│   ├── examforge-core/         # 核心库 crate（可被 TauriExam 引用）
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs          # 公开 API
│   │       ├── pdf_engine.rs   # PDF 渲染 + 文本提取（pdfium-render）
│   │       ├── progress.rs     # 进度追踪（无状态文件扫描）
│   │       ├── ai/
│   │       │   ├── mod.rs      # AiDriver trait 定义
│   │       │   ├── copilot.rs  # Copilot CLI 后端（subprocess）
│   │       │   └── openai.rs   # OpenAI 兼容 API 后端（ureq/reqwest）
│   │       ├── prompt.rs       # 提示词模板组装
│   │       ├── md_parser.rs    # Markdown 解析与校验（regex）
│   │       ├── sqlite_writer.rs # 统一 SQLite 入库（rusqlite）
│   │       ├── config.rs       # TOML 配置管理
│   │       └── models.rs       # Question / Batch / Option 数据模型（serde）
│   │
│   └── examforge-cli/          # CLI 二进制 crate
│       ├── Cargo.toml
│       ├── build.rs            # 复制 pdfium.dll 到 target/
│       └── src/
│           └── main.rs         # clap CLI 入口（from-pdf / generate / import / status）
│
├── prompts/                    # SKILL 提示词模板（嵌入二进制 via include_str!）
│   ├── vision_system.md        # PDF 视觉读题 system prompt
│   ├── generate_system.md      # AI 生成 system prompt
│   └── question_template.md    # Markdown 输出模板
│
└── tests/
    ├── fixtures/               # 测试用 Markdown / PDF 样本
    ├── test_pdf_engine.rs
    ├── test_md_parser.rs
    └── test_sqlite_writer.rs
```

### 关键架构决策

```
1. Workspace 双 crate 结构：
   - examforge-core：纯库，不含 main()，可被 TauriExam 直接引用
   - examforge-cli：薄壳，只做命令行参数解析 + 调 core
   → 未来 TauriExam 可以 `examforge-core = { path = "../examforge/crates/examforge-core" }` 直接嵌入

2. PDFium 捆绑策略：
   - 与 TauriExam 共用同一个 pdfium.dll
   - build.rs 在编译时复制到 target/ 旁边
   - 运行时通过 pdfium-render 动态加载

3. 提示词嵌入：
   - prompts/*.md 通过 include_str!("../../prompts/xxx.md") 编译进二进制
   - 零外部文件依赖，单 exe 即可运行
   - 也支持 --prompt-file 覆盖（方便调试/定制）

4. 数据模型共享：
   - models.rs 定义 Question / Batch / Option 等 struct
   - #[derive(Serialize, Deserialize)] 同时用于 JSON 输出和 SQLite 读写
   - 与 TauriExam 的 Rust 端数据结构保持一致
```

---

## 九、实施路线图

### Phase 1：脚手架 + 统一入库

```
目标：搭建 Rust workspace，实现 examforge import 命令替代两个 Python 入库脚本

工作：
  □ 创建 examforge/ workspace（Cargo.toml + core + cli 双 crate）
  □ 实现 models.rs — Question / Batch / Option 统一数据模型
  □ 实现 md_parser.rs — 用 regex 解析 vision-md 和 exam-gen 两种 Markdown 格式
  □ 实现 sqlite_writer.rs — rusqlite 写入，兼容 TauriExam schema
  □ 实现 CLI: examforge import <dir> --exam SC-100 --mode vision|exam-gen --reset
  □ 验证：对现有 output/vision-md/ 和 output/exam-gen/AI-900/ 入库结果与 Python 脚本一致
  □ cargo build --release → 单文件 examforge.exe

交付物：examforge.exe import 可完全替代两个 Python 脚本
```

### Phase 2：PDF 处理引擎

```
目标：用 Rust 原生完成 PDF 渲染和文本提取，消除 Python/PyMuPDF 依赖

工作：
  □ 实现 pdf_engine.rs — 封装 pdfium-render
    - render_pages(pdf, range, scale) → Vec<PathBuf>  渲染 PNG
    - extract_text(pdf, range) → String  提取文本层
    - page_count(pdf) → usize
  □ build.rs 复制 pdfium.dll 到 target/（复用 TauriExam 的 build.rs 逻辑）
  □ 实现 progress.rs — 扫描 output 目录反推进度（替代 detect_next_batch.py）
  □ 实现 CLI: examforge status --pdf SC-100.pdf
  □ 验证：渲染质量与 PyMuPDF 1.6x 一致

交付物：PDF 处理零 Python 依赖
```

### Phase 3：AI 调度 + generate 命令

```
目标：examforge generate AI-900 --count 50 一个命令自动循环生成

工作：
  □ 实现 ai/mod.rs — AiDriver trait
      trait AiDriver {
          fn complete(&self, prompt: Prompt) -> Result<String>;
          fn complete_with_images(&self, prompt: Prompt, images: &[PathBuf]) -> Result<String>;
      }
  □ 实现 ai/copilot.rs — 调 copilot CLI（std::process::Command）
  □ 实现 ai/openai.rs — OpenAI 兼容 API（ureq POST + base64 图片）
  □ 实现 prompt.rs — 读取 include_str! 嵌入的 SKILL prompt 模板，动态填充参数
  □ 实现 config.rs — 读写 ~/.examforge/config.toml（API Key / 模型 / 端点）
  □ 实现 CLI: examforge generate <exam> --count N --ai copilot|openai --model X
  □ 自动循环：生成→MD校验→写文件→进度更新→入库

交付物：一行命令批量生成任意考试的模拟题
```

### Phase 4：from-pdf 一键化

```
目标：examforge from-pdf SC-100.pdf 一个命令从头到尾

工作：
  □ 实现 CLI: examforge from-pdf <pdf> --exam X --ai copilot|openai
  □ 整合 pdf_engine + progress + ai_driver + md_parser + sqlite_writer
  □ 自动循环：渲染→文本提取→组装 prompt（含 base64 图片）→AI 读题→MD 校验→写文件→入库
  □ Carryover 自动管理：读取上一批 Markdown 尾部，注入下一轮 prompt
  □ 断点续接：crash 后重启自动从最后完成的批次继续
  □ --dry-run 预览模式

交付物：用户提供一个 PDF，拿到一个 SQLite 题库
```

### Phase 5：体验打磨

```
  □ indicatif 进度条（已处理页数 / 总页数 / 已生成题数）
  □ 彩色终端输出（console crate）
  □ tracing 结构化日志 + --verbose
  □ examforge config set ai.backend openai 交互式配置
  □ 错误重试（AI 超时 / 网络断开 / 格式不合规自动重试）
  □ GitHub Release 自动构建（cargo-dist 或 cross）
  □ Shell 补全生成（clap_complete）
```

---

## 十、与 TauriExam 的集成

```
当前关系：
  ExamForge CLI (生产端) → SQLite 题库文件 → TauriExam (消费端)

文件级集成（Phase 1 即可用）：
  1. ExamForge 输出的 SQLite 直接放入 TauriExam 的 question-banks/ 目录
  2. PDF 文件同名放置：question-banks/SC-100.sqlite + question-banks/SC-100.pdf
  3. TauriExam 自动扫描识别

Rust 库级集成（Phase 3+ 可做）：
  因为 examforge-core 是独立 library crate，TauriExam 可以直接引用：

  # TauriExam/src-tauri/Cargo.toml
  [dependencies]
  examforge-core = { path = "../../examforge/crates/examforge-core" }

  这样 TauriExam 内部就能直接调用：
  - examforge_core::md_parser::parse()       → 导入 Markdown
  - examforge_core::pdf_engine::render()     → 复用 PDF 渲染
  - examforge_core::ai::openai::complete()   → 在桌面应用内直接调 AI
  - examforge_core::sqlite_writer::write()   → 入库

  用户在 TauriExam 界面里就能：
  - 拖入 PDF → 自动处理 → 题库出现在列表中
  - 选择考试代号 → AI 生成题库 → 直接开始练习
  - 无需打开终端，无需安装额外工具

共享资源：
  - pdfium.dll：CLI 和 TauriExam 用同一份
  - SQLite schema：完全一致，零适配
  - 数据模型：examforge-core::models 直接复用
```

---

## 十一、关键设计决策总结

| 决策 | 选择 | 理由 |
|---|---|---|
| **CLI 语言** | **Rust** | 单二进制零依赖、与 TauriExam 同栈、可编译为 library crate 嵌入桌面应用 |
| PDF 处理 | pdfium-render | TauriExam 已验证，渲染质量好，支持文本提取，跨平台 |
| SQLite | rusqlite bundled | TauriExam 已验证，bundled 模式无需系统安装 SQLite |
| HTTP/AI | ureq（同步） | 轻量无 tokio 依赖；如需流式再加 reqwest |
| CLI 框架 | clap 4 derive | Rust 生态标准，自动帮助/补全/子命令 |
| Markdown 中间层 | 保留 | 质量闸口，人可审阅，AI 不直接写数据库 |
| AI 后端 | 双模式 | Copilot CLI 快速上手 + OpenAI API 完全可控 |
| 进度管理 | 无状态文件扫描 | 可中断可恢复，不需要额外状态数据库 |
| SQLite schema | 保持 TauriExam 兼容 | 零改造即可被 TauriExam 消费 |
| 每批题数 | 固定 10 | 控制 AI 输出长度，保证质量 |
| 图片传输 | base64 in vision API | OpenAI 兼容模式标准做法 |
| 配置 | TOML 文件 + 环境变量 | 简单、标准、安全（API Key 不入代码） |
| 提示词 | include_str! 嵌入 | 编译进二进制，单 exe 可运行，也可 --prompt-file 覆盖 |
| 分发 | 单文件 .exe | cargo build --release → 一个 examforge.exe + 一个 pdfium.dll |
| 与 TauriExam 集成 | library crate | examforge-core 可被 TauriExam Cargo.toml 直接引用 |

---

## 十二、Cargo.toml 依赖参考

```toml
# examforge/crates/examforge-core/Cargo.toml
[package]
name = "examforge-core"
version = "0.1.0"
edition = "2021"

[dependencies]
clap = { version = "4", features = ["derive"] }       # 仅 cli crate 需要
pdfium-render = "0.9"                                  # PDF 渲染
image = { version = "0.25", default-features = false, features = ["png"] }
base64 = "0.22"                                        # 图片 → base64
rusqlite = { version = "0.32", features = ["bundled"] } # SQLite
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"                                           # 配置文件
regex = "1"                                            # Markdown 解析
ureq = { version = "2", features = ["json", "tls"] }   # HTTP AI API
sha2 = "0.10"                                          # 文件哈希
hex = "0.4"
chrono = { version = "0.4", features = ["serde"] }     # 时间戳
uuid = { version = "1", features = ["v4"] }            # ID 生成
indicatif = "0.17"                                     # 进度条
console = "0.15"                                       # 彩色输出
tracing = "0.1"                                        # 日志
tracing-subscriber = "0.3"
thiserror = "2"                                        # 错误类型

[profile.release]
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

以上依赖与 TauriExam 高度重合（pdfium-render / rusqlite / serde / base64 / image / ureq / sha2 / chrono / uuid），依赖树增量很小。
