---
name: exam-question-gen
description: 'Use when: generating Microsoft certification exam question banks from scratch. Prompts like 随机生成/生成题目/出题/自测题/模拟题, specifying exam codes like AI-900, AZ-104, AZ-305, SC-200, DP-900 etc. Default 10 questions per Markdown file (user can override), structured for self-testing.'
argument-hint: 'Required: exam code (e.g. AI-900, AZ-104). Optional: topic filter, difficulty, batch count, language.'
---

# Microsoft Exam Question Generator

## Goal

基于微软官方认证课程大纲，AI 原创生成高质量模拟题，输出为结构化 Markdown 题库文件（默认 10 题/文件，用户指定数量时按用户的来），用于自测和复习。

题目完全由 AI 生成（无中生有），但必须严格对齐微软官方考试大纲的知识域和技能权重。

## When to Use

Use this skill for prompts such as:

- "随机生成 AI-900 的题目"
- "生成 AZ-104 模拟题"
- "出 10 道 SC-200 的题"
- "帮我生成自测题"
- "生成一批 DP-900 题目"
- "再来一批"
- "继续出题"
- "生成 HOTSPOT 题"
- "出难一点的题"
- "按某个主题出题"

### Regenerate Mode

Use this mode for prompts such as:

- "重新生成 Q5"
- "Q5 质量不好，重做"
- "重写第 5、8 题"
- "Q12 解析不对，重新出"

Procedure:

1. **Locate** — `grep -rn "## Question 5" output/exam-gen/<exam-code>/` to find the Markdown file containing the target question(s).
2. **Regenerate** — Create a new question covering the same knowledge domain and difficulty, with a fresh scenario. Do not reuse the old question stem or options.
3. **Replace** — Overwrite only the `## Question <n>` block in the existing Markdown file (from `## Question <n>` to the line before the next `## Question` or end of file). Keep all other questions untouched.
4. **Re-import** — Run the import script with `--reset` to rebuild the entire SQLite database:
   ```powershell
   python .github/skills/exam-question-gen/scripts/import_question_md_to_sqlite.py --input output/exam-gen/<exam-code> --exam <exam-code> --db question-banks/<exam-code>.sqlite --reset
   ```
5. **Sync** — Copy the updated `.sqlite` to AppData if needed.

Do NOT create a new Markdown file for regenerated questions. Edit in-place in the original file.

## Core Principles

1. **对齐官方大纲**
   - 每道题必须能映射到该考试的某个 Skills Measured 领域。
   - 各领域出题比例应大致符合官方权重（如 AI-900: AI 工作负载 15-20%、ML 原则 20-25% 等）。
   - 题目考察的知识点必须是该认证实际会考的内容。

2. **题型多样化**
   - 支持：single_choice、multiple_choice、hotspot、drag_drop、yes_no_series
   - 默认以 single_choice 为主（约 60%），搭配其他题型。
   - 用户可指定题型偏好。

3. **难度分级**
   - `easy`: 概念识别、定义回忆
   - `medium`: 场景应用、方案选择（默认）
   - `hard`: 多约束条件、最佳实践权衡、陷阱选项

4. **答案必须可靠**
   - 每题必须给出正确答案和中文解析。
   - 解析要引用具体的 Azure 服务/功能行为，而非泛泛而谈。
   - 如果涉及近期更名（Azure AD → Microsoft Entra ID 等），使用最新名称。

5. **干扰项要合理**
   - 错误选项必须是"看起来合理但有明确错误原因"的，不能是明显无关的垃圾选项。
   - 干扰项应体现常见误解或易混淆概念。

## Batch Procedure

### 1. 确定考试范围

根据用户指定的考试代号，确认：
- 考试全称
- Skills Measured 各领域及权重
- 当前有效的产品/服务名称

常见考试参考：

| 代号 | 全称 | 主要领域 |
|---|---|---|
| AI-900 | Azure AI Fundamentals | AI 概念、ML、CV、NLP、生成式 AI |
| AZ-900 | Azure Fundamentals | 云概念、Azure 服务、安全/治理、定价 |
| AZ-104 | Azure Administrator | 身份治理、存储、计算、网络、监控 |
| AZ-305 | Azure Solutions Architect | 治理、计算/网络、存储、监控、BCDR |
| SC-100 | Cybersecurity Architect | 零信任、GRC、安全运维、数据/应用安全 |
| SC-200 | Security Operations Analyst | Sentinel、Defender XDR、KQL |
| DP-900 | Azure Data Fundamentals | 数据概念、关系/非关系数据、分析 |
| DP-203 | Azure Data Engineer | 数据存储、处理、安全、监控 |

### 2. 生成题目

每批默认生成 **10 道题目**（用户指定数量时按用户的来），遵循以下规则：

- 各题分布在不同知识域（除非用户指定某一主题）。
- 至少包含 1 道非标准选择题（hotspot/drag_drop/yes_no_series），除非用户只要选择题。
- 题干应设置清晰的场景（公司名、需求、约束条件），不能只是"以下哪个是...？"。
- 选项 4 个为主（A/B/C/D），多选题明确标注"选择 N 个"。

### 3. 输出 Markdown

使用 [question-batch-template.md](./assets/question-batch-template.md) 模板。

输出路径：

```text
output/exam-gen/<exam-code>/questions-<batch>-<start>-<end>.md
```

例如：`output/exam-gen/AI-900/questions-001-010.md`

### 4. 批次续接

- 检查 `output/exam-gen/<exam-code>/` 下已有文件，自动编号续接。
- 用户说"再来一批"或"继续出题"时，自动生成下一批文件（题数沿用上一批的数量，默认 10）。
- 避免与已生成的题目重复（检查已有文件中的题干关键词）。

## Question Template Structure

每道题必须包含：

```markdown
## Question <n>

- Exam: <exam-code>
- Topic: <knowledge domain>
- Type: single_choice | multiple_choice | hotspot | drag_drop | yes_no_series
- Difficulty: easy | medium | hard

### Question

<场景化题干>

### Options

A. <option>
B. <option>
C. <option>
D. <option>

### Correct Answer

<答案>

### 解析（中文）

<为什么选这个答案，涉及哪个 Azure 服务/功能的什么特性。错误选项为什么错。>

### Key Concept

<一句话总结这道题考察的核心知识点，方便复习索引。>
```

对于 HOTSPOT / DRAG DROP / ordered-list 题型，数据层统一抽象为“候选项池 + 有序目标区”。`Answer Area` 可以保留给人读，但程序判分以 `Interaction Options` / `Interaction Targets` 为准。UI 可以把同一份数据渲染成下拉框、拖拽槽位或排序步骤：

```markdown
### Answer Area

| 提示/位置 | 正确选择 |
|---|---|
| <prompt 1> | <answer 1> |
| <prompt 2> | <answer 2> |
```

结构化交互区必须额外输出：

```markdown
### Interaction Options

| Option ID | Text | Group | Distractor | Sort |
|---|---|---|---|---:|
| opt-a | <candidate option text> | <optional group> | No | 1 |
| opt-b | <candidate option text> | <optional group> | No | 2 |
| opt-c | <distractor option text> | <optional group> | Yes | 3 |

### Interaction Targets

| Target ID | Position | Label | Option Group | Correct Option ID |
|---|---:|---|---|---|
| target-1 | 1 | <first blank / first row / first slot> | <optional group> | opt-a |
| target-2 | 2 | <second blank / second row / second slot> | <optional group> | opt-b |
```

结构化交互约束：

- `Option ID` 必须稳定且唯一，推荐 `opt-a`、`opt-b`、`opt-c`。
- `Target ID` 必须稳定且唯一，推荐 `target-1`、`target-2`。
- `Position` 表示右侧目标区顺序，从 1 开始连续。
- `Correct Option ID` 必须引用 `Interaction Options` 中存在的 `Option ID`。
- 判分只比较 ID，不比较显示文本。
- 干扰项必须保留在 `Interaction Options` 中，并标记 `Distractor = Yes`。
- HOTSPOT 下拉题如每个下拉框候选项不同，用不同 `Group` 和 `Option Group`。
- 排序题右侧无文本时，`Label` 使用 `Step 1`、`Step 2`、`Step 3`。
- 第一版默认每个 option 最多使用一次，不生成需要重复使用同一 option 的题。

对于 Yes/No Series 题型：

```markdown
### Statements

| 陈述 | 正确判断 |
|---|---|
| <statement 1> | Yes / No |
| <statement 2> | Yes / No |
| <statement 3> | Yes / No |
```

## Quality Rules

1. **不出超纲题** — 严格在官方 Skills Measured 范围内。
2. **不出过时题** — 使用最新产品名称和当前行为。
3. **场景化** — 每题至少有一个业务场景或技术约束条件。
4. **解析充分** — 中文解析至少说明正确答案的原因和 1-2 个干扰项为什么错。
5. **无重复** — 同一批次内不出考察完全相同知识点的题。

## Completion Checklist

每批完成前确认：

- [ ] Markdown 文件已创建在 `output/exam-gen/<exam-code>/` 下
- [ ] 包含正好 N 个 `## Question` 标题（N = 用户指定数量或默认 10）
- [ ] 每题有完整的 Options/Answer Area + Correct Answer + 解析
- [ ] 题型分布合理
- [ ] 知识域覆盖多个领域
- [ ] 文件命名连续且无冲突

## SQLite Import

本 skill 自带导入脚本，保持 skill 目录可迁移：

```powershell
python .github/skills/exam-question-gen/scripts/import_question_md_to_sqlite.py --input output/exam-gen/<exam-code> --exam <exam-code> --db question-banks/<exam-code>.sqlite --reset
```

导入脚本职责：读取本 skill 生成的 Markdown，写入 TauriExam 使用的 SQLite schema，包括 `questions`、`options`、`answer_areas`、`interaction_options`、`interaction_targets`，并向后兼容写入 `drag_options`、`drag_slots`、`hotspot_options`、`hotspot_rows`。根目录旧导入脚本暂时保留，不作为本 skill 的必需依赖。
