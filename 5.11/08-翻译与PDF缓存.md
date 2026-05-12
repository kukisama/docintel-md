# 翻译与 PDF 缓存

## 翻译最终方向

对考试题来说，一题内容大小可控，当前更适合采用“一题一条翻译 JSON”。

不必坚持字段级 segment 作为第一设计。

## 推荐表：question_translations

```sql
question_translations
- id
- bank_id
- question_id
- language
- source_hash
- provider
- model
- status
- content_json
- created_at
- updated_at
- unique(question_id, language, source_hash)
```

## content_json 建议结构

```json
{
  "question_text": "...",
  "options": { "A": "...", "B": "..." },
  "answer_areas": [ ... ],
  "interaction_options": { "opt-a": "..." },
  "interaction_targets": { "target-1": "..." },
  "source_answer": "...",
  "recommended_answer": "...",
  "reasoning": "...",
  "notes": "..."
}
```

## 为什么保留 source_hash

`source_hash` 用来判断源题是否变化。

如果题干、选项、交互目标或答案解释变化，hash 变化，旧翻译自动失效。

## 翻译存储位置

第一版可选：

```text
/data/banks/<bank>/translations.sqlite
```

或直接放进：

```text
/data/banks/<bank>/bank.sqlite
```

推荐先单独放 `translations.sqlite`，便于重建、清理和替换。

## PDF 转 JPG 缓存原则

PDF 转 JPG 应该只在服务器上做一次。

张三触发某页渲染后，李四访问同一页时复用基础缓存。

## 推荐缓存目录

```text
/data/banks/SC-100/pages/base/page-001-scale2.jpg
/data/banks/SC-100/pages/base/page-002-scale2.jpg
/data/cache/watermark/<user>/<page>/<hash>.jpg
```

## 缓存 key

推荐 key：

```text
bankVersion + pdfSha256 + pageNumber + scale + cropKey + renderEngineVersion
```

只要 PDF 或渲染参数变了，就自动换缓存。

## 请求流程

```text
GET /api/questions/{id}/source-pages?page=123
  1. 验证用户登录
  2. 验证题库权限
  3. 验证 page=123 属于该题允许页码
  4. 查 pages/base 是否已有基础图
  5. 没有则从 source.pdf 渲染一次并保存
  6. 对基础图叠加用户水印
  7. 返回水印图或裁剪图
```

## 基础图和水印图的区别

基础图：

- 无水印。
- 服务端私有。
- 所有用户共享。
- 长期缓存。

水印图：

- 带用户信息、时间、题库版本。
- 可短 TTL 缓存。
- 可以每次动态生成。

## 不建议

- 不建议把 PDF 每次请求都重新渲染。
- 不建议把完整 PDF 发给前端。
- 不建议把所有 JPG 存进数据库。
- 不建议用户端缓存大量 PDF 页图。
