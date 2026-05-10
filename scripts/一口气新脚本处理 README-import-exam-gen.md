# import_exam_gen_to_sqlite.py

将 AI 生成的 Markdown 题库文件批量导入 SQLite，供 TauriExam 考试工具加载使用。

## 前置条件

- Python 3.10+（无第三方依赖，仅用标准库）
- Markdown 题库文件已通过 `exam-question-gen` Skill 生成在 `output/exam-gen/<EXAM>/` 目录下

## 快速开始

```powershell
# 从 AI-900 题库目录生成 SQLite（输出到 question-banks/AI-900.sqlite）
python scripts/import_exam_gen_to_sqlite.py --input output/exam-gen/AI-900
```

## 参数说明

| 参数 | 必填 | 默认值 | 说明 |
|---|---|---|---|
| `--input` | ✅ | — | 包含 `questions-*.md` 文件的目录路径 |
| `--db` | — | `question-banks/<EXAM>.sqlite` | SQLite 输出路径，默认根据目录名自动推断 |
| `--reset` | — | `false` | 加上此参数会清空表后重新导入 |

考试代号从 `--input` 的最后一级目录名自动提取（如 `output/exam-gen/AI-900` → `AI-900`）。

## 常用命令

```powershell
# 首次导入
python scripts/import_exam_gen_to_sqlite.py --input output/exam-gen/AI-900

# 重新生成题目后清空重建
python scripts/import_exam_gen_to_sqlite.py --input output/exam-gen/AI-900 --reset

# 指定输出路径
python scripts/import_exam_gen_to_sqlite.py --input output/exam-gen/AZ-104 --db question-banks/AZ-104.sqlite

# 其他考试
python scripts/import_exam_gen_to_sqlite.py --input output/exam-gen/SC-200
python scripts/import_exam_gen_to_sqlite.py --input output/exam-gen/DP-900
```

## 输入格式要求

脚本扫描 `--input` 目录下所有 `questions-*.md` 文件，每个文件预期包含 10 道题，结构如下：

```markdown
## Question 1

- Exam: AI-900
- Topic: Azure AI Services
- Type: single_choice
- Difficulty: medium

### Question
<题干>

### Options
A. ...
B. ...
C. ...
D. ...

### Correct Answer
B

### 解析（中文）
<中文解释>

### Key Concept
<核心知识点>
```

支持的题型：`single_choice`、`multiple_choice`、`hotspot`、`drag_drop`、`yes_no_series`

HOTSPOT/DRAG DROP 题使用 `### Answer Area` 表格，Yes/No 题使用 `### Statements` 表格。

## 输出

生成的 `.sqlite` 文件包含 3 张表，与 TauriExam 完全兼容：

- **questions** — 题目主体（题干、答案、解析、题型等）
- **options** — 选择题选项（A/B/C/D 拆分存储）
- **answer_areas** — HOTSPOT/DRAG DROP/Yes-No 的表格答案行

## 在 TauriExam 中使用

将生成的 `.sqlite` 文件复制到以下任一位置即可被自动识别：

```
question-banks/AI-900.sqlite          ← 项目根目录
TauriExam/question-banks/AI-900.sqlite  ← TauriExam 子目录
```

打开 TauriExam 后刷新题库列表即可看到新的考试。

## 完整工作流

```
1. 用 Copilot 触发 exam-question-gen Skill → 生成 Markdown 题库
   "随机生成 AI-900 的题目"  →  output/exam-gen/AI-900/questions-001-010.md
   "再来一批"               →  output/exam-gen/AI-900/questions-011-020.md
   ...

2. 运行本脚本 → 打包成 SQLite
   python scripts/import_exam_gen_to_sqlite.py --input output/exam-gen/AI-900

3. TauriExam 加载 → 做题
   question-banks/AI-900.sqlite → TauriExam 自动识别
```
