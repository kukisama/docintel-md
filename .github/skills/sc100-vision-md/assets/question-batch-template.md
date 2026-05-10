# SC-100 视觉读取整理：Questions <start>–<end>

- 来源 PDF：`<pdf>`
- 处理页范围：PDF pages <from>–<to>
- 读取方式：PDF 页面渲染为图片后进行视觉核对，并用 PDF 文本层辅助抄录。
- 交付粒度：10 个题目 / 1 个 Markdown 文件
- 质量策略：普通选择题直接整理；HOTSPOT/DRAG DROP/图表题保留答案区结构并标记 `needs_review`。
- Carryover：<说明最后一页是否遗留半题、解释或评论；下一批如何衔接。>

## 目录

| 序号 | 原题号 | 页范围 | 题型 | 状态 | 我的建议 |
|---:|---:|---|---|---|---|
| 1 | <q> | <pages> | <type> | <status> | <answer> |

---

## Question <n>

- Source pages: <pages>
- Topic: <topic>
- Type: single_choice | multiple_choice | hotspot | drag_drop | case_study
- Status: parsed | needs_review | version_sensitive | carryover

### Question

<题干，按视觉页整理，去掉水印/页码/广告。>

### Options

A. <option A>  
B. <option B>  
C. <option C>  
D. <option D>

<!-- HOTSPOT/DRAG DROP 使用下面结构替代 Options：
### Answer Area

| Prompt | Correct selection |
|---|---|
| <row> | <selection> |
-->

### Source Answer

<源题库显示答案，保留原始口径。>

### My Recommended Answer

<我认为正确的答案。若与 Source Answer 不同，明确说明。>

### 我的判断（中文）

<中文解释：为什么选这个答案；是否有版本变化风险；是否需要复核。>

### Reasoning

<英文或中英混合依据，可包含 Microsoft Learn 概念、页面视觉证据和评论区高票解释。>

### Notes

- Visual evidence: page <x> shows <evidence>.
- Warnings: <needs_review/version_sensitive/carryover if any>.
