# 核心资产保护与 API 边界

## 核心原则

普通用户不能拿到完整：

- PDF。
- 题库 SQLite。
- 翻译 SQLite。
- 全量题目 JSON。

用户只能通过前端/API 使用当前被授权的题目资源。

## 不是靠隐藏路径

安全边界不是“用户猜不到文件地址”。正确设计是：

> 即使用户知道服务器内部目录、文件名、版本号和存储规则，也不能直接下载完整 PDF/SQLite。

## 文件不暴露为静态资源

不能暴露：

```text
/data/banks/**
/banks/**
/files/**
/download/**
```

反向代理只暴露：

```text
前端静态资源
/api/**
```

`/data/banks/SC-100/source.pdf` 只是容器/宿主机文件路径，不是 HTTP URL。

## API 访问模式

普通用户请求应类似：

```text
GET /api/banks
POST /api/exam-sessions
GET /api/exam-sessions/{id}/current-question
GET /api/questions/{id}/source-pages?page=123
POST /api/exam-sessions/{id}/answers
```

每次 API 请求都应检查：

1. 用户是否登录。
2. 用户是否有该题库权限。
3. 当前题是否属于该题库。
4. 当前页是否属于该题允许查看的页。
5. 当前请求是否超过速率或批量抓取阈值。

## 不提供的能力

普通用户 API 不提供：

```text
GET /download/SC-100.sqlite
GET /download/SC-100.pdf
GET /download/all-questions.json
GET /api/questions?pageSize=999999
```

管理员/作者如果需要导出，也必须走单独权限、审计和水印。

## client-manifest 规则

普通用户可见的 `client-manifest` 只能返回：

- 题库名称。
- 考试代码。
- 版本号。
- 题数。
- 用户权限摘要。
- 前端功能开关。

不能返回：

- `bankSqlite`。
- `sourcePdf`。
- 完整路径。
- hash。
- Blob/S3/MinIO key。
- SAS/签名 URL。
- 可推导下载地址。

注意：不返回路径只是减少信息泄露，真正的保护来自私有存储和 API 鉴权。

## PDF 访问方式

普通用户看 PDF 原文时，服务端返回的是：

- 当前题对应页图。
- 或当前题裁剪图。
- 加水印的图片。
- 短 TTL 响应。

不是源 PDF。

## 对象存储时的规则

如果未来用 MinIO/S3/Azure Blob：

- bucket/container 必须 private。
- 不公开读。
- 不给普通用户完整 PDF/SQLite 的签名 URL。
- 如需签名 URL，只能签单页图/裁剪图，短 TTL。

## 内部转发规则

可以用 Nginx `X-Accel-Redirect` 或类似机制优化文件传输，但必须：

```text
用户请求 API
  -> API 完成登录校验
  -> API 完成 ACL 校验
  -> API 完成题目和页码范围校验
  -> API 内部转发单页图/裁剪图
```

不能内部转发完整 PDF/SQLite 给普通用户。
