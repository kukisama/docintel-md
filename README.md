# docintel-md

`docintel-md` 是一个很小的 Rust 命令行工具，用来直接调用 Azure Document Intelligence REST API，把 PDF 或图片识别结果导出为 Markdown、JSON 和可选的图片/图块 OCR 清单。

> 说明：这里的“图片/图块”指 Azure Document Intelligence 在结果里识别出的 `<figure>...</figure>` 区域及其 OCR 文本/描述素材。本工具目前不裁剪或下载图片二进制文件；如果需要真正的图片文件，请在 PDF 处理流水线中额外导出原图。

## 配置

在项目目录创建或编辑 `.env`，也可以把 `.env` 放在编译后的 exe 旁边：

```text
DOCINTEL_ENDPOINT=https://<resource-name>.cognitiveservices.azure.com/
DOCINTEL_KEY=<your-key>
DOCINTEL_CLOUD=global
DOCINTEL_API_VERSION=2024-11-30
DOCINTEL_MODEL=prebuilt-layout
DOCINTEL_FIGURE_MODE=separate
```

程序会从“当前工作目录”和“exe 所在目录”开始，向上查找 `.env`。因此常见的 `target/release/docintel-md.exe` 可以直接找到项目根目录的 `.env`，不需要把密钥复制到 `target/release/`。

如果使用 21V / Azure 中国区，请使用资源实际显示的 endpoint，并设置：

```text
DOCINTEL_CLOUD=21v
```

## 使用方法

最简用法：

```powershell
.\target\release\docintel-md.exe analyze --input ..\GAL4_User_Manual_ADLDS.pdf
```

大 PDF 建议先本地拆分测试，再分块提交：

```powershell
.\target\release\docintel-md.exe split --input ".\SC-100 题目+答案+讨论.pdf" --pages-per-chunk 200
.\target\release\docintel-md.exe analyze --input ".\SC-100 题目+答案+讨论.pdf" --split-pages 200
```

`split` 只会在本地生成 PDF chunk，不调用 Azure；`analyze --split-pages <n>` 会把 PDF 按每 `n` 页拆分，逐个 chunk 提交给 Document Intelligence，并在输出目录生成合并后的 `*.document-intelligence.combined.md`。

把大型 Markdown 机械切分成单题片段，方便后续逐题交给 AI 抽取 JSON：

```powershell
.\target\release\docintel-md.exe segment --input ".\output\document-intelligence\SC-100 题目+答案+讨论\SC-100 题目+答案+讨论.document-intelligence.combined.md" --exam SC-100
```

`segment` 会识别 `# Question #1` 这类题号标题，输出 `segments\sc-100-0001-q0001.md` 等单题片段，并生成 `manifest.json` 记录处理序号、原题号、题号重复次数、行号、内容 hash 和处理状态。这个阶段不调用 AI，适合先检查切题边界是否可靠。

从切分结果机械抽取前 40 题为一题一个 JSON：

```powershell
.\target\release\docintel-md.exe extract-json --manifest ".\output\question-pipeline\SC-100 题目+答案+讨论.document-intelligence.combined\manifest.json" --from 1 --limit 40
```

`extract-json` 会使用全局处理序号作为稳定主键，同时保留 OCR 中的原题号和重复次数。对于截图题、HOTSPOT、选项/答案无法可靠识别的题目，会把 `status` 标记为 `needs_review` 并写入 `warnings`，方便后续逐题交给 AI 或人工复核。

把视觉整理后的标准 Markdown 导入 SQLite：

```powershell
C:/Users/kukisama/AppData/Local/Programs/Python/Python312/python.exe scripts/import_vision_md_to_sqlite.py --reset
```

默认读取 `output\vision-md\**\sc-100-questions-*.md`，生成 `output\vision-db\sc-100.sqlite`。数据库会保留批次目录信息、原题号、页码范围、题型、状态、完整题目 Markdown、题干、选项、HOTSPOT/DRAG DROP 答案区、源答案、我的推荐答案、中文判断、推理和备注。`page_from` / `page_to` 可用于在应用中展开 PDF 对应页或预渲染页面图片。

指定输出目录：

```powershell
.\target\release\docintel-md.exe analyze --input ..\GAL4_User_Manual_ADLDS.pdf --output ..\output\document-intelligence\GAL4_User_Manual_ADLDS
```

所有关键配置也可以通过参数传入：

```powershell
.\target\release\docintel-md.exe analyze --input .\file.pdf --endpoint "https://<resource>.cognitiveservices.azure.com/" --key "<key>" --cloud global --output .\out
```

## 图片/图块处理模式

代码现在至少支持下面三种模式，可用 `--figure-mode` 或 `.env` 中的 `DOCINTEL_FIGURE_MODE` 设置：

| 模式 | 参数值 | 行为 |
| --- | --- | --- |
| 带图一起输出 | `inline` | 把 Document Intelligence 返回的 `<figure>` OCR 文本改写成易读的代码块，并保留在主 Markdown 中。 |
| 忽略图输出 | `ignore` | 从主 Markdown 中移除 `<figure>` OCR 文本，不生成单独的图片 OCR 清单；完整原始数据仍在 JSON 中。 |
| 带图，图片描述单独存储 | `separate` | 从主 Markdown 中移除 `<figure>` OCR 文本，并把每个图片/图块的 OCR 文本写入 `*.document-intelligence.figures.md`。这是默认模式。 |

示例：

```powershell
.\target\release\docintel-md.exe analyze --input .\file.pdf --figure-mode inline
.\target\release\docintel-md.exe analyze --input .\file.pdf --figure-mode ignore
.\target\release\docintel-md.exe analyze --input .\file.pdf --figure-mode separate
```

## 输出文件

以 `file.pdf` 为例，默认 `separate` 模式会在输出目录生成：

```text
file.document-intelligence.md
file.document-intelligence.json
file.document-intelligence.figures.md
file.document-intelligence.meta.json
README.md
```

各文件用途：

- `*.document-intelligence.md`：来自 `analyzeResult.content`，请求参数使用 `outputContentFormat=markdown`；根据图片模式决定是否包含图片/图块 OCR。
- `*.document-intelligence.json`：完整保留服务端返回结果，适合后续提取表格、页码、版面、置信度等信息。
- `*.document-intelligence.figures.md`：仅在 `separate` 模式生成，用于存放图片/图块 OCR 文本和页码。
- `*.document-intelligence.meta.json`：记录输入文件、模型、API 版本、图片模式、耗时和输出文件名。
- `README.md`：本次输出目录的中文说明文件。

## 推荐处理流程

```text
document-intelligence.md          # 主 OCR/layout Markdown
document-intelligence.figures.md  # 图片/截图/图块 OCR 转录（仅 separate 模式）
document-intelligence.json        # 完整页面、表格、figure 布局数据
+ 原始导出的图片文件             # 如果 PDF 流水线能额外导出
=> AI 清理与重写 => final .2nd.md
```

生成最终 Markdown 时，不建议把所有图片 OCR 原样塞回正文。更好的做法是：用图片 OCR 清单辅助撰写图注、步骤说明或可搜索转录，并把真正重要的图片放在相应位置。
