# TauriExam 运行时与双模式

## 当前运行时事实

TauriExam 当前优先使用：

```text
%LOCALAPPDATA%\TauriExam
```

题库运行时目录：

```text
%LOCALAPPDATA%\TauriExam\question-banks
```

用户记录库：

```text
%LOCALAPPDATA%\TauriExam\app.sqlite
```

## 自动迁移逻辑

当前 TauriExam 会在首次启动时做一次迁移：

1. 创建 `%LOCALAPPDATA%\TauriExam`。
2. 创建 `%LOCALAPPDATA%\TauriExam\question-banks`。
3. 如果目标题库目录已经有 SQLite，则不迁移。
4. 如果目标题库目录为空，从旧位置找第一个有题库的目录复制进去。

旧位置大致包括：

```text
exe 旁边的 question-banks
当前工作目录 question-banks
TauriExam/question-banks
根目录 question-banks
```

所以它是“一次性补种子”，不是持续同步。

## 未来必须区分双模式

### 作者/内部模式

配置建议：

```text
EXAM_CLIENT_MODE=local-author
```

行为：

- 可以读取完整本地 SQLite。
- 可以读取完整本地 PDF。
- 可以打开本地题库目录。
- 用于作者制题、审题、验证、内部调试。

### 普通用户在线模式

配置建议：

```text
EXAM_CLIENT_MODE=cloud-user
```

行为：

- 必须 AAD/OIDC 登录。
- 只调用服务端 API。
- 不读取完整本地题库目录。
- 不下载完整 SQLite/PDF。
- 只缓存当前题、下一题、当前页图等短 TTL 数据。

## 前端 API Adapter 方向

当前：

```text
React -> Tauri invoke -> Rust command -> 本地 SQLite/PDF
```

未来：

```text
local-author: React -> Tauri invoke -> 本地 Rust command -> 本地 SQLite/PDF
cloud-user:   React -> HTTP fetch  -> Rust exam-api     -> 服务端 SQLite/PDF/cache
```

## 改造难度判断

难度中等，不是推倒重来。

可复用：

- React/Vite 前端。
- 题目面板。
- Drag Drop / Hotspot 组件。
- 大部分 TypeScript 类型。
- 判分显示和交互状态管理思路。

需要改：

- `api.ts` 要支持 Tauri adapter 和 HTTP adapter。
- 后端核心逻辑要从 `src-tauri/src/main.rs` 抽到 Rust library。
- Web 版不能暴露本地目录、打开文件夹、完整题库下载等能力。
