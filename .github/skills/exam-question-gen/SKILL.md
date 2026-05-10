---
name: exam-question-gen
description: 'Use when: generating Microsoft certification exam question banks from scratch. Prompts like 随机生成/生成题目/出题/自测题/模拟题, specifying exam codes like AI-900, AZ-104, AZ-305, SC-200, DP-900 etc. Outputs 10 questions per Markdown file, structured for self-testing.'
argument-hint: 'Required: exam code (e.g. AI-900, AZ-104). Optional: topic filter, difficulty, batch count, language.'
---

# Microsoft Exam Question Generator

## Goal

基于微软官方认证课程大纲，AI 原创生成高质量模拟题，输出为结构化 Markdown 题库文件（10 题/文件），用于自测和复习。

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

每批生成 **10 道题目**，遵循以下规则：

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
- 用户说"再来一批"或"继续出题"时，自动生成下一个 10 题文件。
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

对于 HOTSPOT / DRAG DROP 题型：

```markdown
### Answer Area

| 提示/位置 | 正确选择 |
|---|---|
| <prompt 1> | <answer 1> |
| <prompt 2> | <answer 2> |
```

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
- [ ] 包含正好 10 个 `## Question` 标题
- [ ] 每题有完整的 Options/Answer Area + Correct Answer + 解析
- [ ] 题型分布合理
- [ ] 知识域覆盖多个领域
- [ ] 文件命名连续且无冲突
