# sc100-vision-md Skill

将 SC-100 考试 PDF 通过**视觉读取**转换为结构化 Markdown 题库。默认每次处理 10 道题，用户指定数量时按用户的来。

## 触发方式

在 VS Code Copilot Chat 中直接说以下任意一句：

| 触发词 | 说明 |
|---|---|
| `继续处理` | 自动检测上次进度，接着处理下一批 |
| `继续` / `接着做` / `下一批` | 同上，最常用的触发 |
| `继续处理题库` | 同上 |

### 重生成模式

当某道题质量不好或答案有问题时，可以指定重新处理：

| 触发词 | 说明 |
|---|---|
| `重新生成 Q23` | 重新视觉读取 PDF 并重写该题 |
| `Q23 质量不好，重做` | 同上 |
| `重写第 23、32 题` | 同时重新处理多道题 |

重生成流程：定位 MD 文件 → 重新视觉读取源 PDF 页 → 原地替换 Question 块 → `--reset` 重新导入。不会创建新 MD 文件。

**不需要指定页码** — skill 会自动运行 `detect_next_batch.py` 检测已处理进度，决定下一批的起始页。

如果需要指定范围，可以说：
- `处理 SC-100 的 101-130 页`
- `从第 200 页开始处理`

## 工作流程

```
用户说"继续" → detect_next_batch.py 检测进度
    ↓
已全部完成？→ 直接报告 done，不做任何操作
    ↓ 否
render_pdf_pages.py 渲染 PDF 页面为图片
    ↓
extract_page_text.py 提取辅助文字
    ↓
AI 视觉读取每一页图片（不盲信 OCR）
    ↓
生成 Markdown（10 题/文件）
    ↓
记录 carryover（如有跨页题目）
```

## 批量自动处理

使用根目录的 `continue-sc100.ps1` 可以无人值守连续处理多轮：

```powershell
.\continue-sc100.ps1 -Rounds 35 -Model gpt-5.5 -Effort medium
```

每轮自动检测进度 → 处理 10 题 → 处理完毕自动停止。

### 批量补充结构化数据

如果大量 hotspot/drag_drop 题缺少 `Interaction Options` / `Interaction Targets`，用根目录的 `batch-add-interactions.ps1` 批量补充：

```powershell
.\batch-add-interactions.ps1 -Rounds 30 -Model gpt-5.5 -Effort medium
```

每轮自动检测下一个缺失批次 → Copilot 视觉读 PDF 页面 → 原地修改 MD 文件插入结构化表格 → 全部完成后统一导入 SQLite。

## 输出位置

| 内容 | 路径 |
|---|---|
| 渲染页面图片 | `output/vision-pages/sc-100-pages-<from>-<to>/` |
| 提取的辅助文字 | 同上目录，`page-text-*.txt` |
| Markdown 题库 | `output/vision-md/sc-100-pages-<from>-<to>/sc-100-questions-<start>-<end>.md` |

## 题目结构

每道题包含：
- **Source Answer** — PDF 原始答案
- **My Recommended Answer** — AI 推荐答案
- **我的判断（中文）** — 中文解析和推理
- **Status** — `parsed` / `needs_review` / `version_sensitive` / `carryover`

### 结构化交互题（HOTSPOT / DRAG DROP / 排序题）

复杂题型输出统一的 `Interaction Options` + `Interaction Targets` 结构化数据，用于程序判分和 TauriExam UI 渲染。同时保留 `Answer Area` 表格供人阅读。

## 导入 SQLite

处理完成后，用 skill 自带脚本导入到 TauriExam 数据库：

```powershell
python .github/skills/sc100-vision-md/scripts/import_question_md_to_sqlite.py `
  --input output/vision-md --exam SC-100 `
  --db question-banks/SC-100.sqlite --reset
```

## 文件结构

```
.github/skills/sc100-vision-md/
├── SKILL.md                          # Skill 指令（Copilot 读取）
├── README.md                         # 本文件（使用说明）
├── assets/
│   ├── question-batch-template.md    # Markdown 模板
│   └── structured-interactions.md    # 结构化交互规则
└── scripts/
    ├── detect_next_batch.py          # 自动检测下一批
    ├── detect_missing_interactions.py # 检测缺失结构化数据的题
    ├── render_pdf_pages.py           # PDF → 图片
    ├── extract_page_text.py          # PDF → 辅助文字
    └── import_question_md_to_sqlite.py  # Markdown → SQLite
```
