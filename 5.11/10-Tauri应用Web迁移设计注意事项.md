# Tauri 应用 Web 迁移设计注意事项

## 核心判断

Tauri 应用如果一开始设计得好，迁移到 Web 并不难；真正难的不是 UI，而是：

- 业务逻辑写死在 Tauri command 里。
- 前端到处直接调用 `invoke()`。
- 数据读写默认依赖本机文件系统。
- 权限、缓存、API 边界没有提前抽象。

所以从第一天开始就要把 Tauri 当作“一个客户端壳”，而不是把它当成唯一运行形态。

## 推荐总体结构

```text
frontend-react
  ├─ components/          # 纯 UI，尽量不关心 Tauri/Web
  ├─ features/            # 业务页面和状态
  ├─ api/                 # API adapter 层
  │   ├─ types.ts
  │   ├─ client.ts
  │   ├─ tauriClient.ts
  │   └─ httpClient.ts
  └─ platform/            # 平台能力抽象
      ├─ platform.ts
      ├─ tauriPlatform.ts
      └─ webPlatform.ts

rust
  ├─ exam_core            # 业务核心库
  ├─ exam_tauri           # Tauri commands
  └─ exam_api             # Web HTTP API
```

关键原则：

```text
UI 不直接知道 Tauri
业务逻辑不锁死 Tauri
数据模型不绑定本地文件
Tauri command 和 HTTP API 共用 exam_core
```

## 前端最重要：封装 API adapter

不要在组件里到处写：

```text
invoke('get_question', ...)
```

应该统一封装：

```text
api.getQuestion(bankId, questionId)
api.getInteractionModel(bankId, questionId)
api.submitAnswer(sessionId, answer)
api.getSourcePage(questionId, page)
```

然后底层分两个实现：

```text
tauriClient: api.getQuestion -> invoke('get_question')
httpClient:  api.getQuestion -> fetch('/api/questions/...')
```

这样迁移 Web 时，组件不用大改，只替换 adapter。

## 组件要保持“纯 UI”

组件应该接收数据和回调，而不是自己决定数据从哪里来。

推荐：

```text
<QuestionPanel
  detail={detail}
  interaction={interaction}
  onSubmit={submitAnswer}
/>
```

不推荐：

```text
QuestionPanel 内部直接 invoke('get_question')
QuestionPanel 内部直接读本地文件路径
QuestionPanel 内部直接判断 AppData 目录
```

## 平台能力要隔离

Tauri 有一些 Web 没有的能力：

- 打开本地文件。
- 选择目录。
- 访问 AppData。
- 调用系统命令。
- 本地 SQLite。
- 本地 PDF 渲染。

这些能力都应该放到 `platform` 层。

示例：

```text
platform.openExternal(url)
platform.pickFile()
platform.getAppPaths()
platform.saveLocalDraft()
```

Web 版没有的能力要提供替代行为：

- 隐藏按钮。
- 提示“仅作者本地模式支持”。
- 改为调用服务端 API。

## 数据模型要面向 API，而不是面向本地文件

前端类型应该描述业务数据：

```text
BankInfo
QuestionDetail
InteractionModel
ExamSession
SourcePageImage
TranslationResult
```

不要把本地路径当成前端核心类型：

```text
db_path
pdf_path
local_file_path
appdata_dir
```

这些路径可以存在于作者/内部模式，但普通用户 Web/API 模型里不应该出现。

## Tauri 后端不要承担全部业务逻辑

不推荐：

```text
src-tauri/src/main.rs
  - 查询 SQLite
  - PDF 渲染
  - 判分
  - 翻译
  - AI
  - 题库导入
  - 缓存
  - 权限
  - 所有逻辑都在这里
```

推荐：

```text
exam_core
  - 查询 SQLite
  - 判分
  - interaction model
  - PDF 渲染接口
  - 翻译数据模型

exam_tauri
  - 把 Tauri command 转成 exam_core 调用

exam_api
  - 把 HTTP endpoint 转成 exam_core 调用
```

这样未来不会“Web 重写一遍后端”。

## 本地模式和云模式从一开始区分

建议配置：

```text
EXAM_CLIENT_MODE=local-author
EXAM_CLIENT_MODE=cloud-user
```

### local-author

允许：

- 读取本地完整 SQLite。
- 读取本地完整 PDF。
- 打开本地题库目录。
- 导入/验证题库。

适合：作者、审题、内部测试。

### cloud-user

只允许：

- 登录。
- 请求题库列表。
- 按题请求数据。
- 按题请求页图。
- 提交答案和学习记录。

不允许：

- 打开本地题库目录。
- 下载完整 SQLite。
- 下载完整 PDF。
- 访问完整翻译库。

## 鉴权不要后补

如果未来要 Web/AAD 登录，最好从 API 设计阶段就把用户上下文放进去。

所有关键 API 都应该天然带用户上下文：

```text
getBanks(user)
getQuestion(user, bankId, questionId)
getSourcePage(user, questionId, page)
submitAnswer(user, sessionId, answer)
```

不要先写成无用户版本，后面再到处补权限判断。

## 缓存策略要按平台区分

### local-author

可以缓存：

- 完整题库。
- 完整 PDF。
- 页图。
- 翻译库。
- 导入中间产物。

### cloud-user

只能缓存：

- 当前题。
- 下一题少量预取。
- 当前页图短 TTL。
- 用户自己的作答草稿。

不能缓存：

- 全量题库。
- 全量 PDF。
- 全量翻译库。

## PDF 渲染要提前抽象

当前 Tauri 可以本地 PDFium 渲染；Web 版不能直接读服务端私有 PDF。

推荐抽象成：

```text
getSourcePage(questionId, page): SourcePageImage
```

Tauri local-author 实现：

```text
本地 PDFium -> base64/image
```

Web/cloud-user 实现：

```text
HTTP API -> 服务端渲染/缓存/水印后的图片
```

前端组件只关心拿到图片，不关心图片来自本地还是服务端。

## 路由和状态不要绑定桌面语义

Web 需要 URL、刷新、回退、分享链接等能力。

建议从一开始就让页面状态接近 URL 状态：

```text
/banks/:bankId/questions/:questionId
/sessions/:sessionId
/review/wrong?bankId=SC-100
```

Tauri 里也可以用同样的前端路由，只是运行在 WebView 里。

## 常见坑

### 坑 1：组件里直接 invoke

后果：Web 迁移时每个组件都要改。

解决：统一 API adapter。

### 坑 2：前端依赖本地路径

后果：Web 版没有路径概念，权限也会混乱。

解决：普通业务类型不暴露本地文件路径。

### 坑 3：Tauri main.rs 变成巨型后端

后果：Web API 要重写一遍。

解决：抽 `exam_core`。

### 坑 4：本地缓存默认等于完整题库

后果：普通用户版保护不了 PDF/SQLite。

解决：local-author 和 cloud-user 缓存策略分开。

### 坑 5：先无权限，后补权限

后果：后面会漏接口、漏分页、漏文件下载。

解决：API 函数签名从一开始带 user/context。

### 坑 6：PDF 渲染耦合 UI

后果：Web 版无法复用。

解决：统一 `getSourcePage()` 数据接口。

### 坑 7：把“隐藏路径”当安全

后果：路径泄露就能下载。

解决：文件不暴露为静态资源，必须走 API 鉴权。

## 对当前 TauriExam 的建议

下一步可以按这个顺序改：

1. 把 `src/api.ts` 明确改造成 adapter 接口。
2. 建立 `tauriClient.ts`，封装现有 `invoke()`。
3. 预留 `httpClient.ts`，先不必完全实现。
4. 把 `BankInfo` 中仅本地使用的 `db_path`、`pdf_path` 标注为 local-only。
5. 把 PDF 页图获取抽象成 `getSourcePage()`。
6. 把复杂题判分和 interaction model 抽进 Rust `exam_core`。
7. 新增 `EXAM_CLIENT_MODE` 概念，前端按模式隐藏能力。

## 最终口径

Tauri 最容易迁移 Web 的设计方式是：

```text
Tauri = 本地平台适配器
React = 可复用 UI
exam_core = 可复用业务核心
exam_api = Web 服务端入口
api adapter = 切换 Tauri/Web 的关键层
```

只要从一开始守住这个边界，Tauri 到 Web 是“换运行入口和 API adapter”，不是重写整个应用。
