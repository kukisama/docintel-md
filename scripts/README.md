# scripts 目录说明

这个目录现在只保留通用辅助脚本和历史兼容脚本。

## 当前推荐

- `Resolve-Python.ps1`：PowerShell 调用 Python 工具前的统一解析器；优先复用已安装 Python，缺失时可通过 winget 静默安装 Python 3.12。
- `azure-linux-devbox-bootstrap.sh`：Azure Ubuntu 开发机初始化脚本；用于 Azure Portal Run Command/自定义脚本扩展，配置 SSH `65444`，并补齐 VS Code Remote SSH 常见依赖与基础目录初始化。脚本不会安装远端 VS Code 桌面版，首次 Remote SSH 连接时会自动下发 VS Code Server。

## Deprecated

以下旧导入器已废弃，仅作历史参考：

- `import_vision_md_to_sqlite.py`
- `import_exam_gen_to_sqlite.py`

请改用 skill 内新版导入器：

- `.github/skills/sc100-vision-md/scripts/import_question_md_to_sqlite.py`
- `.github/skills/exam-question-gen/scripts/import_question_md_to_sqlite.py`

新版导入器支持当前 TauriExam 使用的统一 `interaction_options` / `interaction_targets` 结构，并兼容生成旧的 drag/hotspot 表。
