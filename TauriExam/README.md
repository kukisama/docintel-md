# TauriExam

基于 Tauri + React + TypeScript 的本地考试练习工具，题目来源为项目根目录下已经生成的题库 SQLite：

```text
../output/vision-db/sc-100.sqlite
```

## 当前能力

- 读取 SC-100 题库 SQLite。
- 顺序浏览全部题目。
- 搜索题号/题干。
- 按题型、状态筛选。
- 查看题目、选项、HOTSPOT/DRAG DROP 答案区。
- 答案默认隐藏，点击后显示源答案、推荐答案、中文判断和 reasoning。
- 根据 `page_from` / `page_to` 加载已渲染的 PDF 页图片。
- 支持深色 / 浅色主题切换。
- 支持创建简单考试、记录单题耗时和总耗时。
- 历史考试记录保存到独立学习库：

```text
../output/exam-tool/exam-tool.sqlite
```

## 开发运行

```powershell
cd TauriExam
npm install
npm run tauri dev
```

## 构建

```powershell
cd TauriExam
npm run build
npm run tauri build
```

## 数据边界

- 题库 SQLite 只读，来源于标准 Markdown 入库流程。
- 考试历史、错题、收藏、翻译缓存等学习数据写入独立应用 SQLite。
- PDF 原文页第一版使用已有 `../output/vision-pages/**/page-xxx.png` 图片；后续可替换为 Rust/Tauri 内部按需渲染 PDF。

## 后续增强

- 多题库注册管理。
- PDF 页 Rust 原生按需渲染。
- 错题本、收藏、复核标记。
- 原文/译文切换和 Azure Translator 缓存。
- Azure Speech 朗读/听题模式。
