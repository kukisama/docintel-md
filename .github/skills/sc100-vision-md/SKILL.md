---
name: sc100-vision-md
description: 'Use when: continuing SC-100/exam PDF question-bank processing, prompts like 继续处理/继续/下一批/接着做, visual page reading, PDF screenshots, HOTSPOT/DRAG DROP, auto-detect next pages/questions from output/vision-md, 10 questions per Markdown, answer verification, Chinese answer recommendations, carryover across page boundaries.'
argument-hint: 'Optional: PDF path or page range; otherwise auto-detect next batch from existing output/vision-md'
---

# SC-100 Vision-to-Markdown Question Processing

## Goal

Turn an image-heavy SC-100/exam PDF into reliable, review-friendly Markdown batches by visually reading rendered PDF pages, not by blindly trusting OCR or source answers.

The output is **10 questions per Markdown file**. Each question must include the question text, choices or answer area, the source answer, my recommended answer, Chinese reasoning, source pages, status, and carryover notes.

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
- Stop the Markdown after 10 complete questions, not necessarily after the tentative page window.
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
5. For HOTSPOT, represent answers as a Markdown table instead of forcing A/B/C/D choices.

## Output Naming

Recommended output paths:

```text
output/vision-pages/sc-100-pages-<from>-<to>/page-<from>.png
output/vision-pages/sc-100-pages-<from>-<to>/page-text-<from>-<to>.txt
output/vision-md/sc-100-pages-<from>-<to>/sc-100-questions-<start>-<end>.md
```

If a page range does not contain exactly 10 complete questions, name by actual question range.

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
