# exam-question-gen Skill

基于微软官方认证考试大纲，AI **从零生成**高质量模拟题。默认每批 10 题，用户指定数量时按用户的来。输出为结构化 Markdown。

## 触发方式

在 VS Code Copilot Chat 中直接说：

| 触发词 | 说明 |
|---|---|
| `随机生成 AI-900 的题目` | 指定考试代号，生成第一批 |
| `生成 AZ-104 模拟题` | 同上 |
| `出 10 道 SC-200 的题` | 同上 |
| `帮我生成自测题` | 需要明确考试代号 |
| `再来一批` / `继续出题` | 自动编号续接上一批 |
| `生成 HOTSPOT 题` | 指定题型偏好 |
| `出难一点的题` | 指定难度为 hard |
| `按网络主题出 AZ-104 的题` | 按主题筛选 |

### 重生成模式

当某道题质量不好或解析有误时，可以指定重新生成：

| 触发词 | 说明 |
|---|---|
| `重新生成 Q5` | 用新场景重新出同知识域的题 |
| `Q5 质量不好，重做` | 同上 |
| `重写第 5、8 题` | 同时重新生成多道题 |
| `Q12 解析不对，重新出` | 同上 |

重生成流程：定位 MD 文件 → 生成新题（同知识域、新场景） → 原地替换 Question 块 → `--reset` 重新导入。不会创建新 MD 文件。

### 必须参数

- **考试代号**（必填）：AI-900、AZ-900、AZ-104、AZ-305、SC-100、SC-200、DP-900、DP-203 等

### 可选参数

- **主题** — 限定某个知识域
- **难度** — easy / medium（默认）/ hard
- **题型** — single_choice、multiple_choice、hotspot、drag_drop、yes_no_series

## 支持的考试

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

## 工作流程

```
用户指定考试代号 → 确认考试大纲和知识域权重
    ↓
检查 output/exam-gen/<exam-code>/ 已有文件 → 自动编号续接
    ↓
AI 按大纲比例生成 10 道题（场景化题干 + 合理干扰项）
    ↓
输出 Markdown 文件
```

## 批量自动处理

使用根目录的 `continue-exam-gen.ps1` 可以无人值守连续生成多轮：

```powershell
.\continue-exam-gen.ps1 -ExamCode AI-900 -Rounds 20 -Model gpt-5.5 -Effort medium
```

## 输出位置

```
output/exam-gen/<exam-code>/questions-<start>-<end>.md
```

例如：
- `output/exam-gen/AI-900/questions-001-010.md`
- `output/exam-gen/AI-900/questions-011-020.md`

## 题目结构

每道题包含：
- **Exam** / **Topic** / **Type** / **Difficulty**
- **Question** — 场景化题干
- **Options** 或 **Answer Area** + **Interaction Options** / **Interaction Targets**
- **Correct Answer**
- **解析（中文）** — 正确原因 + 干扰项错误原因
- **Key Concept** — 一句话核心知识点

### 结构化交互题（HOTSPOT / DRAG DROP / 排序题）

统一输出 `Interaction Options` + `Interaction Targets`，用于 TauriExam UI 渲染和程序化判分。同时保留 `Answer Area` 供人阅读。

## 导入 SQLite

生成完成后，用 skill 自带脚本导入到 TauriExam 数据库：

```powershell
python .github/skills/exam-question-gen/scripts/import_question_md_to_sqlite.py `
  --input output/exam-gen/AI-900 --exam AI-900 `
  --db question-banks/AI-900.sqlite --reset
```

## 文件结构

```
.github/skills/exam-question-gen/
├── SKILL.md                          # Skill 指令（Copilot 读取）
├── README.md                         # 本文件（使用说明）
├── assets/
│   └── question-batch-template.md    # Markdown 模板
└── scripts/
    └── import_question_md_to_sqlite.py  # Markdown → SQLite
```
