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
