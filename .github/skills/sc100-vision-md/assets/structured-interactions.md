# Structured Interaction Recognition Rules

本文件用于指导 SC-100 / PDF 视觉读题时如何处理可程序判分的复杂题型。目标是：不要把所有截图题都粗暴写成普通 `Answer Area`；只要视觉上能确认候选项、槽位、下拉框或顺序，就输出结构化 Markdown，供导入脚本落库和 TauriExam UI 自动判分。

## 1. 统一抽象：候选项池 + 有序目标区

大多数考试里的 HOTSPOT / DRAG DROP / 排序题，数据层都可以归一成同一个模型：

1. **左侧候选项池**：视觉上可见的 3、4、5、6 个候选答案，按出现顺序编号。
2. **右侧目标区**：需要填写的 1、2、3... 个目标位，天然有顺序。
3. **正确映射**：每个目标位引用一个候选项 ID。

不同题型只是 UI 展示不同：

| 视觉形态 | 数据含义 | 推荐 Interaction |
|---|---|---|
| 下拉框 HOTSPOT：每个小题一个下拉框 | 有序目标区，每个目标从指定候选组选择一个答案 | `dropdown_hotspot` |
| 普通拖拽：左侧候选项拖到右侧语义槽位 | 有序目标区，每个目标匹配一个候选项 | `drag_drop` |
| 排序题：左侧候选项按 1、2、3 放到右侧 | 有序目标区，目标标签通常是 Step 1/2/3 | `ordered_list` |
| 图片点击题：点击截图/架构图区域 | 无文本候选项池，需坐标或人工视觉说明 | `image_hotspot` |

> 核心规则：只要能从图片中看到候选项池、右侧目标区和正确映射，就不要降级成人工自评。程序判分只需要比较“目标位 ID → 候选项 ID”，并不关心 UI 是下拉框还是拖拽。

## 1.1 推荐统一 Markdown 结构

新题优先输出统一结构。旧字段 `Hotspot Options` / `Hotspot Rows` / `Drag Options` / `Drag Slots` 仍兼容，但 AI 识别时应先按下面这个模型思考：

```markdown
### Interaction Options

| Option ID | Text | Group | Distractor | Sort |
|---|---|---|---|---:|
| opt-a | <candidate text visually read from PDF> | <optional group> | No | 1 |
| opt-b | <candidate text visually read from PDF> | <optional group> | No | 2 |
| opt-c | <unused distractor text> | <optional group> | Yes | 3 |

### Interaction Targets

| Target ID | Position | Label | Option Group | Correct Option ID |
|---|---:|---|---|---|
| target-1 | 1 | <first blank / first row / first slot> | <optional group> | opt-a |
| target-2 | 2 | <second blank / second row / second slot> | <optional group> | opt-b |
```

字段含义：

- `Option ID`：题内稳定 ID。不同题可以重复使用 `opt-a`，但同一题内必须唯一。
- `Group`：用于下拉题中不同小题拥有不同候选项池；普通拖拽/排序题可以统一写一个 group。
- `Distractor`：候选项池里未使用但视觉存在的干扰项必须保留。
- `Target ID`：右侧目标位 ID。
- `Position`：右侧目标位顺序，必须从 1 开始连续。
- `Label`：目标位显示文本。若视觉上没有小题文本，就写 `Step 1`、`Step 2`。
- `Option Group`：该目标位可选的候选项组；如果所有候选项共享同一池，可以留空或写统一 group。
- `Correct Option ID`：引用 `Interaction Options.Option ID`。

UI 可以根据 `Interaction` 选择展示方式，但导入和判分可以归一成同一类结构。

## 1.2 何时仍需特殊处理

只有真正的 **image_hotspot** 例外：如果答案不是文本候选项，而是点击图片/拓扑/Portal 截图上的区域，则没有左侧候选项池。此时第一版输出 `Visual Target Notes`，后续如需自动判分再补百分比坐标。

## 2. dropdown_hotspot：多下拉框选择题

适用场景：

- 一个大题里有多个小题/空格。
- 每个小题像下拉框一样从若干文本项中选择。
- 示例：
  - “To enable Azure AD authentication for App1, use: [dropdown]”
  - “To implement access requests for App1, use: [dropdown]”

Markdown 输出：

```markdown
- Type: hotspot
- Interaction: dropdown_hotspot

### Answer Area

| Prompt | Source selection | My recommended selection |
|---|---|---|
| To enable Azure AD authentication for App1, use | Azure AD application | Azure AD application |
| To implement access requests for App1, use | An access package in Identity Governance | An access package in Identity Governance |

### Hotspot Options

| Option ID | Text | Group | Sort |
|---|---|---|---:|
| hs-auth-a | Azure AD application | auth | 1 |
| hs-auth-b | Azure AD Application Proxy | auth | 2 |
| hs-auth-c | Azure Application Gateway | auth | 3 |
| hs-auth-d | A managed identity in Azure AD | auth | 4 |
| hs-auth-e | Microsoft Defender for App | auth | 5 |
| hs-access-a | An access package in Identity Governance | access | 1 |
| hs-access-b | An access policy in Microsoft Defender for Cloud Apps | access | 2 |
| hs-access-c | An access review in Identity Governance | access | 3 |
| hs-access-d | Azure AD Conditional Access App Control | access | 4 |
| hs-access-e | An OAuth app policy in Microsoft Defender for Cloud Apps | access | 5 |

### Hotspot Rows

| Row ID | Prompt | Option Group | Correct Option ID |
|---|---|---|---|
| row-1 | To enable Azure AD authentication for App1, use | auth | hs-auth-a |
| row-2 | To implement access requests for App1, use | access | hs-access-a |
```

要求：

- `Hotspot Options` 必须包含该下拉框视觉上可见的完整候选项。
- 如果两个下拉框候选项不同，用不同 `Group`，如 `auth`、`access`。
- 如果多个下拉框共享同一组选项，可以使用同一个 `Group`。
- `Correct Option ID` 必须引用 `Hotspot Options.Option ID`。
- `Answer Area` 仍保留给人读，但程序判分以 `Hotspot Options` / `Hotspot Rows` 为准。

## 3. ordered_list：排序/步骤题

适用场景：

- 左侧有候选动作/步骤。
- 右侧答案区要求把若干项按顺序放入。
- 常见界面有上/下箭头、左右移动按钮。
- 例如“选择并按正确顺序排列三个步骤”。

Markdown 输出：

```markdown
- Type: drag_drop
- Interaction: ordered_list

### Drag Options

| Option ID | Text | Group | Distractor |
|---|---|---|---|
| opt-a | Establish ransomware recovery readiness. | actions | No |
| opt-b | Enable additional protection and detection controls. | actions | No |
| opt-c | Establish visibility. | actions | No |
| opt-d | Implement disaster recovery. | actions | Yes |
| opt-e | Enable automation. | actions | Yes |

### Ordered Slots

| Slot ID | Position | Label | Correct Option ID |
|---|---:|---|---|
| slot-1 | 1 | Step 1 | opt-c |
| slot-2 | 2 | Step 2 | opt-b |
| slot-3 | 3 | Step 3 | opt-a |
```

要求：

- 左侧候选项池必须完整抄录。
- 右侧如果只需要放 2 个答案，就只建 2 个 slot；剩余候选项标记为 `Distractor = Yes`。
- `Position` 是判分顺序，必须从 1 开始连续。
- 排序题本质上可以复用 Drag Drop UI，但 UI 应显示成“顺序槽位”。

## 4. drag_drop：普通匹配拖拽题

适用场景：

- 左侧是候选项池。
- 右侧是若干语义目标。
- 不强调全局顺序，而是每个目标匹配一个候选项。

Markdown 输出沿用主 skill 中的：

```markdown
- Type: drag_drop
- Interaction: drag_drop

### Drag Options

| Option ID | Text | Group | Distractor |
|---|---|---|---|
| opt-a | <candidate> | <group> | No |

### Drag Slots

| Slot ID | Label | Correct Option ID |
|---|---|---|
| slot-1 | <target label> | opt-a |
```

## 5. image_hotspot：图片区域点击题

适用场景：

- 正确答案是点击图片、架构图、Azure Portal 截图中的区域。
- 没有可见文本下拉候选项。

第一版不要伪造坐标。输出：

```markdown
- Type: hotspot
- Interaction: image_hotspot
- Status: needs_review

### Answer Area

| Prompt | Source selection | My recommended selection |
|---|---|---|
| Click the component that should be configured | <visible source answer if known> | <recommended visual target> |

### Visual Target Notes

- Page: <page number>
- Target description: <describe the exact region visually, e.g. the Conditional Access policy row>
- Bounding box: TODO-manual-review
```

后续如果要做真正图片点击判分，需要额外记录截图坐标：`x_percent`、`y_percent`、`width_percent`、`height_percent`。当前阶段只保留视觉说明。

## 6. 字段命名约定

每道复杂题都建议在 metadata 增加：

```markdown
- Type: hotspot | drag_drop
- Interaction: dropdown_hotspot | ordered_list | drag_drop | image_hotspot | manual_hotspot
```

`Type` 用于大类过滤；`Interaction` 用于 UI 和导入脚本选择具体控件。

## 7. 状态规则

- `parsed`：候选项池、槽位/行、正确映射都能从视觉页确认。
- `needs_review`：候选项池不完整、答案区被截断、源答案不可信、图片坐标无法精确确认。
- `version_sensitive`：答案依赖产品名称或功能演进。

## 8. 禁止事项

- 不要只输出最终答案而省略候选项池。
- 不要把所有 HOTSPOT 都降级成人工自评。
- 不要凭空补不存在于图片中的候选项。
- 不要用显示文本做程序判分 key；必须使用稳定 ID。
- 不要把 `opt-a` / `hs-auth-a` 当成用户界面主展示文案，它们只是内部 ID。
