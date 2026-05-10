---
name: sc100-vision-md
description: 'Use when: continuing SC-100/exam PDF question-bank processing, prompts like 继续处理/继续/下一批/接着做, visual page reading, PDF screenshots, HOTSPOT/DRAG DROP, auto-detect next pages/questions from output/vision-md, default 10 questions per Markdown (user can override), answer verification, Chinese answer recommendations, carryover across page boundaries.'
argument-hint: 'Optional: PDF path or page range; otherwise auto-detect next batch from existing output/vision-md'
---

# SC-100 Vision-to-Markdown Question Processing

## Goal

Turn an image-heavy SC-100/exam PDF into reliable, review-friendly Markdown batches by visually reading rendered PDF pages, not by blindly trusting OCR or source answers.

The output defaults to **10 questions per Markdown file** unless the user specifies a different number (e.g. "处理 5 道" or "这次 20 题"). Each question must include the question text, choices or answer area, the source answer, my recommended answer, Chinese reasoning, source pages, status, and carryover notes.

## When to Use

Use this skill for prompts such as:

- “继续处理”
- “继续”
- “下一批”
- “接着做”
- “继续处理题库”
- “视觉读题，写 md”
- “从 PDF 图片读题”
- “自动接着上次处理”
- “10 个题目一个 md”
- “HOTSPOT / DRAG DROP / 截图题整理”
- “判断题库答案是否正确”
- “给出中文答案建议和你的看法”

If the user only says “继续处理”, do **not** ask for page numbers. First inspect the existing outputs and decide the next batch yourself.
### Regenerate Mode

Use this mode for prompts such as:

- "重新生成 Q23"
- "Q23 质量不好，重做"
- "重写第 23、32 题"
- "Q42 答案有问题，重新处理"

Procedure:

1. **Locate** — `grep -rn "## Question 23" output/vision-md/` to find the Markdown file containing the target question(s).
2. **Re-read source** — Open the source PDF pages noted in the question's `Source Pages` field. Re-render if needed. Visually re-read the question, options, answer area, and any HOTSPOT/DRAG DROP structure.
3. **Replace** — Overwrite only the `## Question <n>` block in the existing Markdown file (from `## Question <n>` to the line before the next `## Question` or end of file). Keep all other questions untouched.
4. **Re-import** — Run the import script with `--reset` to rebuild the entire SQLite database from all Markdown files:
   ```powershell
   python .github/skills/sc100-vision-md/scripts/import_question_md_to_sqlite.py --input output/vision-md --exam SC-100 --db question-banks/SC-100.sqlite --reset
   ```
5. **Sync** — Copy the updated `.sqlite` to AppData if needed.

Do NOT create a new Markdown file for regenerated questions. Edit in-place in the original file.
## Core Principles

1. **Visual first, OCR second**
   - Render PDF pages to images and inspect the visual page.
   - Use PDF text extraction only as a transcription accelerator.
   - Never rely only on mechanically split Markdown for HOTSPOT, DRAG DROP, tables, diagrams, or screenshots.

2. **Do not blindly trust source answers**
   - Preserve the source answer.
   - Add `My recommended answer` and `我的判断（中文）`.
   - If source answer seems outdated, ambiguous, or contradicted by docs/comments, mark it as `needs_review` or `version_sensitive`.

3. **Carryover is mandatory**
   - If the final page in a batch contains half of a question, answer, explanation, or comments, record carryover.
   - The next batch must prepend carryover context before starting new questions.
   - Do not start a new Markdown question until a visible `Question #...` boundary is found.

4. **Markdown is the quality gate**
   - Keep Markdown clean and human-readable.
   - JSON/SQLite conversion happens only after Markdown is structurally acceptable.

## Batch Procedure

### 0. Auto-detect the next batch when page numbers are omitted

When the user does not provide explicit pages, run [detect_next_batch.py](./scripts/detect_next_batch.py):

```powershell
C:/Users/kukisama/AppData/Local/Programs/Python/Python312/python.exe .github/skills/sc100-vision-md/scripts/detect_next_batch.py
```

Use its JSON result to determine:

- whether all pages are already processed (`done: true`)
- source PDF path
- latest completed Markdown file
- latest processed page range
- latest processed question range
- previous carryover notes
- suggested next page range
- suggested output folders

Default behavior:

- Continue from `latest_page_to + 1`.
- Render a tentative 30-page window.
- Visually inspect actual question boundaries.
- Stop the Markdown after the target number of complete questions (default 10, or user-specified), not necessarily after the tentative page window.
- If the next page starts with continuation from the previous question, attach it to prior carryover notes before starting the new question batch.

Only ask the user for clarification if no PDF exists or no previous output exists and multiple PDFs are present.

**Stop rule:** If `detect_next_batch.py` returns `"done": true`, do not render images, do not extract text, do not create or edit Markdown, and do not ask follow-up questions. Report that processing is already complete and end the turn.

For a user-driven Copilot CLI loop, prefer the tiny root script `continue-sc100.ps1`. It only repeats a single prompt; this skill does the detection, rendering, extraction, visual reading, Markdown writing, and done/no-op stopping:

```powershell
.\continue-sc100.ps1 -Rounds 35 -Model gpt-5.5 -Effort medium
```

The script checks `detect_next_batch.py` before each Copilot CLI round. If `done` is true, it exits without invoking Copilot. It uses non-interactive mode: `copilot --model <model> --effort <level> -p <prompt> --allow-all`.

### 1. Render page images

Use [render_pdf_pages.py](./scripts/render_pdf_pages.py):

```powershell
C:/Users/kukisama/AppData/Local/Programs/Python/Python312/python.exe .github/skills/sc100-vision-md/scripts/render_pdf_pages.py --pdf "<pdf from detect_next_batch>" --from <next_from> --to <next_to> --output <suggested_pages_output>
```

Default render scale is suitable for reading. Increase `--scale` if small tables or diagrams are hard to inspect.

### 2. Extract helper text

Use [extract_page_text.py](./scripts/extract_page_text.py):

```powershell
C:/Users/kukisama/AppData/Local/Programs/Python/Python312/python.exe .github/skills/sc100-vision-md/scripts/extract_page_text.py --pdf "<pdf from detect_next_batch>" --from <next_from> --to <next_to> --output <suggested_text_output>
```

This text is only a helper. For answer areas and screenshots, inspect images.

### 3. Inspect page boundaries

- List visible `Question #...` starts in the extracted text.
- Open/rendered images for every question start page.
- Open all HOTSPOT/DRAG DROP/table/diagram pages.
- Determine whether the last page has carryover.

### 4. Build Markdown

Use [question-batch-template.md](./assets/question-batch-template.md).

Required structure per question:

- Source pages
- Topic
- Type
- Status
- Question
- Options or Answer Area
- Source Answer
- My Recommended Answer
- 我的判断（中文）
- Reasoning
- Notes

### 5. Quality rules

Mark status:

- `parsed`: standard single/multiple choice and visually clear answer.
- `needs_review`: HOTSPOT, DRAG DROP, table/diagram answer, or source answer unclear.
- `version_sensitive`: answer may have changed because Azure/Microsoft product behavior changed.
- `carryover`: batch ended before the current question/comments/explanation fully ended.

## Answer Verification Guidance

When judging correctness:

1. Prefer official Microsoft Learn behavior/current product naming.
2. For old labels, normalize carefully:
   - Azure AD → Microsoft Entra ID
   - Azure Purview → Microsoft Purview
3. If the source answer is plausible but comments disagree, add a warning instead of silently changing it.
4. For SC-100 architecture/security questions, reason from the requirement keywords:
   - “minimize development effort” often points to managed services/workflow automation.
   - “dashboard/custom views” in Sentinel usually points to workbooks.
   - “SOAR/minimize manual intervention” in Sentinel usually points to playbooks.
   - “request access via My Apps + approval” usually points to Identity Governance access packages.
5. For HOTSPOT, do not force A/B/C/D choices. First classify the interaction using [structured-interactions.md](./assets/structured-interactions.md):
   - dropdown-like multi-row HOTSPOT → output `Hotspot Options` and `Hotspot Rows`.
   - ordered steps/list placement → output `Drag Options` and `Ordered Slots`.
   - visual image click target → output `Visual Target Notes` and mark `needs_review` unless coordinates are explicitly captured.
   - only fall back to plain `Answer Area` when the visual structure cannot be confirmed.

## Structured Interaction Rules

Before writing any HOTSPOT, DRAG DROP, ordered-list, table-selection, or screenshot-based question, read [structured-interactions.md](./assets/structured-interactions.md).

Key rule: treat most complex questions as one unified data model: a visually ordered candidate option pool on the left, and an ordered target/answer area on the right. If the PDF image shows enough information to identify candidate options, target rows/slots, and correct mappings, the question is programmatically gradable and must receive a structured section. `Answer Area` is for human readability; structured sections are for SQLite import and UI grading. UI may render the same data as dropdowns, drag/drop slots, or ordered steps.

## Drag Drop Structured Output

DRAG DROP 题必须优先输出结构化拖拽区。`Answer Area` 可以保留给人类阅读，但程序导入和后续 UI 判分以 `Drag Options` / `Drag Slots` 为准。

```markdown
### Drag Options

| Option ID | Text | Group | Distractor |
|---|---|---|---|
| opt-a | <candidate option text visually read from PDF> | <optional group> | No |
| opt-b | <candidate option text visually read from PDF> | <optional group> | No |
| opt-c | <distractor option text visually read from PDF> | <optional group> | Yes |

### Drag Slots

| Slot ID | Label | Correct Option ID |
|---|---|---|
| slot-1 | <drop target / requirement visually read from PDF> | opt-a |
| slot-2 | <drop target / requirement visually read from PDF> | opt-b |
```

DRAG DROP 视觉整理规则：

- 从 PDF 页面视觉区域提取完整候选项池，不能只写最终答案。
- `Option ID` 必须稳定且唯一，推荐 `opt-a`、`opt-b`、`opt-c`。
- `Slot ID` 必须稳定且唯一，推荐 `slot-1`、`slot-2`。
- `Correct Option ID` 必须引用 `Drag Options` 中存在的 `Option ID`。
- 判分只比较 ID，不比较显示文本，避免翻译和标点差异导致误判。
- 如果候选项池、槽位或答案映射无法从视觉页确认，标记 `Status: needs_review`，并不要伪造 `Drag Options` / `Drag Slots`。
- 第一版默认每个 option 最多使用一次；如果原题明确允许重复使用，在 Notes 中说明，暂不自动判分。

## Output Naming

Recommended output paths:

```text
output/vision-pages/sc-100-pages-<from>-<to>/page-<from>.png
output/vision-pages/sc-100-pages-<from>-<to>/page-text-<from>-<to>.txt
output/vision-md/sc-100-pages-<from>-<to>/sc-100-questions-<start>-<end>.md
```

If a page range does not contain exactly the target number of complete questions, name by actual question range.

## SQLite Import

本 skill 自带导入脚本，保持 PDF 读取 skill 目录可迁移：

```powershell
python .github/skills/sc100-vision-md/scripts/import_question_md_to_sqlite.py --input output/vision-md --exam SC-100 --db question-banks/SC-100.sqlite --reset
```

可通过 `--exam`、`--input`、`--pattern`、`--db` 处理非 SC-100 题库或不同输出路径。导入脚本写入 TauriExam 使用的 SQLite schema，包括 `questions`、`options`、`answer_areas`、`drag_options`、`drag_slots`。根目录旧导入脚本暂时保留，不作为本 skill 的必需依赖。

## Completion Checklist

Before finishing:

- If `detect_next_batch.py` returned `done: true`, no files were changed and the turn ended cleanly.
- Rendered page images exist.
- Markdown file exists.
- Markdown has exactly the intended number of `## Question` headings.
- Every question has `My Recommended Answer` and `我的判断（中文）`.
- HOTSPOT/DRAG DROP questions have `Answer Area` tables.
- Carryover is explicitly recorded if needed.
- Output files are under `output/` and ignored by Git.
