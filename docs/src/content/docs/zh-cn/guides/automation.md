---
title: 自动化
description: 把同一个操作一次应用到多个数据集或图，或运行已保存的工作流。
---

当同一个操作需要作用于一整个系列的实验时，自动化用一次可预检的批处理
取代逐个数据集的点击。

选择**文件 → Automation…**（命令面板中也可调用）。窗口分两个标签页：

## Current Project

作用于已打开的内容。搜索并勾选需要的数据集或图——或点击 **Current
selection** 载入当前选择——再选择一个工具，点击 **Preflight** 预览会影响
哪些目标、哪些会被跳过。**Confirm and execute** 执行，整批操作合并为
一次 **Undo automation** 撤销。

### 参数设置

有三个工具能改到对象检查器（**Object inspector**）和处理步骤里的设置，于是
一个层级、一种颜色、一个线宽、一个字号或一种窗函数，都可以经一次可预检的批
处理应用到整个项目：

| 工具 | 作用 |
| --- | --- |
| **Inspect a property** | 读取当前值、默认值，以及该参数允许的范围 |
| **Set a property** | 写入一个值 |
| **Reset a property** | 按当前数据重新推导该值，与面板上的重置按钮一致 |

在 **Parameters (JSON)** 中用 id 指定参数：**Set a property** 用
`{"key": "series.contour.count", "value": 12}`，另外两个用
`{"key": "series.contour.count"}`。

这三个工具够得到各面板能编辑的一切设置：对象与序列的样式、处理步骤的参数、
文档与画布的设置，以及应用偏好。该指定哪个资源由参数本身决定——等高线和线条
参数在图对象上，切趾参数在数据集上，图形排印在文档上（列为
**Plotium document**），页面尺寸在画布上，偏好设置在应用上（列为
**Plotium application**）。

| 对象检查器中的设置 | id | 取值 |
| --- | --- | --- |
| **Lowest level** | `series.contour.base.magnitude` | 大于 0 的数；究竟以什么计量取决于 **Anchor** |
| **Anchor** | `series.contour.base.policy` | `absolute`、`noise_floor`、`background_scale`、`fraction_of_range` |
| **Levels** | `series.contour.count` | 1 到 256 |
| **Level ratio** | `series.contour.ratio` | 大于 1，最大 10 |
| **Negative contours** | `series.contour.negative.enabled` | `true` 或 `false` |
| **Positive colour** | `series.contour.positive_color` | `"#rrggbb"` |
| **Negative colour** | `series.contour.negative_color` | `"#rrggbb"` |
| **Line width** | `series.contour.line_width` | 0.05 到 10 pt；默认 0.5 |
| **Stroke width**（**Line** 区域） | `series.line.stroke_width` | 0.05 到 10 pt；默认 0.5 |
| **Tick-label size**（**Figure typography** 区域） | `document.figure.typography.tick_pt` | 1 到 72 |

应用偏好设置同样用这三个工具。对于
`settings.appearance.accent.color`，**Set a property** 接受 `"#rrggbb"`，把画布
强调色固定为该颜色；**Reset a property** 清除它，强调色重新跟随主题。

| 切趾步骤上的设置 | id | 取值 |
| --- | --- | --- |
| **Window** | `dataset.processing.apodization.kind` | `none`、`cosine_bell`、`exponential`、`gaussian` |
| **LB** | `dataset.processing.apodization.lb_hz` | −10000 到 10000 |
| **GB** | `dataset.processing.apodization.gb_hz` | 大于 0，最大 10000 |

各等高线参数的含义、以及每种锚定方式需要什么样的数据，见
[等高线层级](/zh-cn/guides/contour-levels/)；切趾各行的说明见
[数据处理](/zh-cn/guides/processing/)。

#### 勾选的目标会展开成什么

被勾选的资源往往并不是真正被写入的对象——展开成什么由参数决定：图对象展开
为它的各条序列，数据集展开为它的各个处理步骤，文档就是它自己。预检和结果列
表按展开后的部件逐行列出，因此一个含三条序列的图会给出三行，并指明是哪个图
对象里的哪条序列。

参数够不到的部件会标为 **Skipped** 并给出原因，其余部件照常应用：等高线下方
的热图、参数属于切趾时列表里的零填充步骤，或对一个窗函数为 *Exponential* 的
步骤索取 **GB**，都是这种情况。完全没有可寻址内容的资源（文本框、没有管线的
数据集）同样被跳过。

结果行和运行记录会在资源旁写出部件：

```json
{
  "resource": { "id": "…", "kind": "plotium.dataset" },
  "component": { "kind": "processing_step", "id": 3 }
}
```

序列部件写作 `{"kind": "series", "id": 2}`。两种 id 都只在它所属的资源内有
意义，换一个数据集，同一个步骤 id 就什么也不代表。

#### 为什么某个目标被跳过

每一条被跳过的行都带有一句供人阅读的说明。由写入本身跳过的行还带有
`skip_reason`——一个稳定的标记，工作流据此分支即可，不必去匹配文字：

| `skip_reason` | 含义 |
| --- | --- |
| `already_at_value` | 目标本来就是这个值，没有写入任何内容 |
| `not_applicable` | 该参数不适用于这个目标 |
| `target_missing` | 这个地址在文档里已经指不到任何东西 |

在预检阶段就被排除的行只有那句说明。它们绝不会是「值本来就一样」这种情况，
因此没有这个标记本身就是区分。

如果 **Set a property** 的所有目标本来就是这个值，那么它什么也不会写入：每个
目标都报告为跳过，文档版本号不变，撤销栈上也不会多出一步。

超出范围的值在 **Preflight** 阶段、也就是确认之前就会被拒绝，并同时给出你
填的值和拒绝它的边界。过了这一步，列出的部件要么全部生效，要么全部不变，
写入的改动合并为一次 **Undo automation** 撤销。

**Inspect a property** 把读数返回给调用方：作为工作流的一步运行时，读数会
进入运行记录，[命令行](/zh-cn/reference/cli/)写出的清单文件里也是同一份
内容。在窗口里，数值显示在逐部件结果下方的 **Result value (JSON)** 区域：
每个部件一份读数，包含当前值、默认值和它接受的范围。

#### 一条读数包含什么

每条读数写出自己的 `target`，并带上当前值 `value`、参数有默认值时的
`default_value`、表示当前值是否偏离默认值的 `modified`、可用状态
`availability`，以及约束取值的 `schema`。

当前状态下不允许写入的参数照样会被读出：`availability` 为 `"disabled"`，
`disabled_reason` 说明要先改什么——例如设置 φ0 之前，先把相位模式切到
*Manual*。

schema 以 `type` 标记，取值为 `bool`、`text`、`int`、`stepped_int`、`float`、
`enum` 或 `color`。

- `int` 与 `stepped_int` 带 `min` 与 `max`，参数有单位时还带 `unit`。
  `stepped_int` 另有取值必须落在的格点 `step`——例如 Savitzky-Golay 窗口为
  3 到 201，步长为 2。
- `float` 带 `min`、`max` 与 `exclusive_min`；若参数拒绝某些取值，还带
  `excluded`（单个取值）或 `excluded_magnitude`（绝对值不超过该阈值的全部
  取值）。其 `display`（`linear`、`degrees` 或 `log10`）说明面板如何显示这个
  数；旁边的 `unit` 与 `log` 只是同一件事的另一种写法。
- `enum` 列出各变体，每个都带稳定 id 和标签。

无论 `display` 是什么，边界和取值始终使用参数自身的单位：`display` 为
`degrees` 的相位值，在线格式中仍是弧度。

## External Inputs

运行一个从磁盘文件开始的已保存工作流——例如：导入文件夹里的每个实验、
应用一个处理配方、逐个导出图形。点击 **Open workflow…** 载入，
**Validate** 校验，再点 **Confirm and run workflow**。进度会逐步显示，
较长的运行可以取消。

## 什么会被记录下来

一次工作流运行会留下一份记录：工作流本身及其哈希、Plotium 版本，以及每一步
作用的目标、参数和结果。在这里运行，记录保存在项目中；用
[命令行](/zh-cn/reference/cli/)运行，同样的记录写成文件。**Current
Project** 里的批处理不会留下这种记录——它就是一次普通编辑，在文档里，也在
撤销栈上。

任何向项目之外写文件的操作（例如导出图形）还会列在 **Help → Operation and
Diagnostic History** 中，附带每个文件的路径、大小和 SHA-256 校验和。中途
失败的导出也会列出此前已经写出的文件，落到磁盘上的内容不会无从追查。

工作流文件是纯 JSON，也可以脱离桌面应用运行——见
[命令行](/zh-cn/reference/cli/)，它以无界面方式执行同样的工作流。工作流
与运行记录文件本身的说明见[文件格式](/zh-cn/reference/file-formats/)。

## 本机 agent 连接（MCP）

打开 **Preferences → Agent connections**。菜单 **File → Agent connections…**
和命令面板中的 **Agent connections** 都会跳转到同一个设置页面。
点击 **Start connection**。

### 连接 Codex CLI

1. 安装 Codex CLI 并登录（须支持 `--no-daemon`；已用 0.163.0 版本验证）。
2. 在 Plotium 中点击 **Copy Codex launch command**。
3. 将命令原样粘贴到终端执行：macOS 和 Linux 使用 zsh 或 bash，Windows
   使用 PowerShell。命令自动填入当前地址和密钥，启动新的 Codex 会话，
   无需手动替换内容，也无需另行执行 `export` 或 `codex mcp add`。
4. 在 Codex 中输入 `/mcp`，确认 `plotium` 已连接且有十一个工具。
   可以先发送：“使用 plotium 查看当前 Plotium 项目，暂时不要修改。”

命令使用临时启动配置和仅对本次调用生效的密钥环境变量，并启动独立 Codex
进程，避免已有后台服务沿用旧环境。命令不写入 Codex 配置文件。请在希望 Codex
工作的目录中执行。停止并重新开启 Plotium 连接后，需要重新复制命令并启动
新的 Codex 会话。

### 其他 MCP 客户端

**Copy connection information** 提供含有实际 `url` 和认证 `headers` 的
通用 JSON 对象。外部客户端须支持 HTTP MCP 和自定义认证头，其结构如下：

```json
{
  "url": "http://127.0.0.1:PORT/mcp",
  "headers": { "Authorization": "Bearer TOKEN" }
}
```

复制内容已经填入实际端口和密钥；不同客户端的外围配置可能不同。Plotium
不修改其他软件的设置。每次开启均分配新的本机端口和随机密钥，不保存连接
配置和授权。关闭设置页面不会停止连接；停止连接、切换项目或退出会撤销密钥。
仅接受本机连接；浏览器请求的 Origin 必须与服务自身来源一致，不开放跨域访问。

### 权限与支持范围

连接默认使用 **Read and modify**（读取和修改），agent 连接后即可执行支持的
修改，并可通过 Plotium 的 **Undo** 与 **Redo** 撤销和重做。
在 **Access** 中选择 **Read only**（只读）会立即阻止后续修改，包括待执行计划；
表格查询及既有编辑预检仍可用；导入预检要求读取和修改权限。访问权限适用于所有持有该连接密钥的客户端，停止连接后
失效；重新开启连接时恢复默认的 **Read and modify**。

实际 Agent 项目读取和执行均保存运行记录，使项目进入未保存状态；握手、工具
发现及静态属性目录查询不生成运行记录。

连接支持 CSV/TSV 仅数据导入、原生表格查询、画布预览、文档结构、显式数值曲线绑定和
资源查询、数据集／画布／画布对象重命名，以及以下显示属性：

| 属性 ID | 目标 |
| --- | --- |
| `series.line.stroke_width` | 图对象中的线图序列 |
| `series.contour.line_width` | 图对象中的等高线序列 |
| `series.contour.count` | 图对象中的等高线序列 |
| `document.figure.typography.tick_pt` | `document` |
| `document.figure.typography.label_pt` | `document` |

此接口暂不支持处理参数、应用偏好、其他格式导入、计算、导出、内置聊天及
stdio 连接。

### 不依赖名称发现结构

先用 `{}` 调用 `plotium_get_state`。其 `connection_capabilities` 描述当前连接的
十一个工具、输入 schema、结果约定、支持的操作、属性 ID、输出限制和当前执行权限。
标准 MCP `tools/list` 也提供工具输入 schema。资源描述中的能力属于 **core** 的
数据和渲染语义；出现 `figure.export` 或 `data.transform` 不代表此连接已开放该操作。
这里可预检 `resource.rename`、`properties.set`、`properties.reset`，以及下述受限的
`data.import` 模式及 `plot.series.bind`／`plot.series.materialize_default`。
只读权限允许表格查询及既有属性预检，但拒绝导入／绑定预检和所有修改执行。

使用返回的 revision 列举文档画布：

```json
{
  "expected_revision": 7,
  "target": {"resource":{"id":"document","kind":"plotium.document"}},
  "view": "children",
  "offset": 0,
  "limit": 50
}
```

将此请求传给 `plotium_document_inspect`。复制完整的 `value.items[].target`
作为下一次请求目标，依次进入画布、面板、内容和 Series。画布先列面板，再列未归属
面板的内容；面板内的内容只在所属面板下列出。使用 `view: "self"` 并省略分页参数，
读取目标详情、`ancestors` 归属链和 `child_count`。内容类型包括图、文字、形状和
栅格图片，不返回图片像素或数据行。frame 使用 pt：面板相对页面，面板内内容相对
面板；画布尺寸使用 mm。

Series 发现与属性寻址一样，使用图对象的当前显示绑定。非当前通道或显示模式的
替代 Series 仍保存在项目中，但在重新激活前不列出、也不可寻址。当前绑定中由
用户隐藏的 Series 仍会列出，`visible` 为 false。切换通道或显示模式后，须读取
当前 revision，再发现当前有效目标。

Series 详情包含可直接用于属性查询的完整 target、编码、可见性、显式标签，以及
`source` 中的数据集和字段资源引用、所属数据集内的 `field_id`、可选 `item_id`、
单位和绑定状态。用返回的资源 ID 调用 `plotium_resources_inspect`，可读取字段和
列的描述。字段失效时保留 `field_id`，但不提供可解析的字段引用。来源状态包括
`resolved`、`missing_dataset`、`missing_field`、`missing_item`、
`incompatible_encoding`、`unbound` 和 `missing_column`。

表格的 `binding_mode: "table_default"` 返回共享的 `x_column` 和全部
`series_columns` Y／误差列绑定。一个逻辑 Series 可能展开为多条表格显示曲线，
这些显示曲线不会因此获得独立 Series ID。
显式来源返回 `binding_mode: "table_explicit"`、TableId、当前表 revision、
`x_column`、`y_column` 和可选 `error_column`，不把默认值误报为其来源。
`coordinate_units` 返回图坐标单位契约。
名称（包括同名曲线）仅作显示，不能代替身份。面板地址为
`CANVAS_UUID/panels/PANEL_ID`；内容与 Series 保持下文既有地址。

子项查询默认每页 50 项，允许 1–200，返回 `total`、`next_offset` 和 `truncated`。
保持**相同 revision 和 target**，按 `next_offset` 继续读取；256 KiB 结构化响应
预算可能使一页提前结束。详情或单个条目超过预算时返回 `response_too_large`。
不提供递归展开整个项目的接口。revision 变化会拒绝续页，须重读状态并重新列举。
目标失效或身份伪造返回 `invalid_parameters`，不会改用当前选择。

`plotium_properties_list {}` 只返回当前五项静态定义，包括 `schema`、`unit`
（无量纲时为空）、`scope`、`access` 和静态 `applicability`，不混入当前目标值。
用 `plotium_properties_inspect` 查询明确目标的当前值、默认值、范围和适用性。
属性是否适用与连接是否允许修改分别表达。

### 导入 CSV/TSV 数据，不创建图

让 agent 导入明确指定的 UTF-8 CSV/TSV，检查列信息，再分页读完整张表。先调用
`plotium_get_state {}`，然后调用 `plotium_plan`：

```json
{
  "expected_revision": 7,
  "tool_id": "data.import",
  "parameters": {
    "mode": "table_data_only",
    "paths": ["/absolute/path/measurement.csv"],
    "header": "present"
  }
}
```

导入不提供 `ids` 或 `targets`。`header` 可为默认的 `auto`、`present` 或 `absent`；
自动判断有歧义则失败。`.csv` 与 `.tsv` 分别选择逗号和制表符；可显式用
`format: "csv" | "tsv"` 指定。只读所列普通文件，不扫描目录、不自动寻找邻接
sidecar、不回写源文件、不创建图或 X/Y 绑定。

预检在后台完成准备后才返回。`import.items` 逐文件报告 `prepared` 或 `failed`，
包含拟建原生 ID、元数据和诊断，此时项目尚未新增数据集。确认结果后调用
`plotium_execute {"plan_id":"RETURNED_PLAN_ID"}`。执行使用预检冻结的字节和 typed
snapshot，不重新读取可能已变化的文件；提交前重验 revision 和权限，将全部已准备
成功项作为一次原子修改提交。`value.status` 为 `succeeded`、`partial` 或 `failed`，
须逐项查看结果和已创建 ID；失败文件不留下半成品。一次 Undo 移除本批成功项，
Redo 恢复同一 DatasetId、FieldId、ColumnId 和 RowId。既有 GUI/CLI 导入行为不变。

元数据包含 DatasetId、TableId、表 RevisionId、字段身份、稳定 ColumnId、逻辑
类型、nullable、单位、总行数及诊断。字段和列是不同的原生身份。缺失值保持 null，
混合数值/文本列保留文本；超出有符号 64 位范围的整数字样保留文本，不舍入。
不根据数值或表头拼写猜单位。`type_source` 区分 `file/user/inferred/unknown`，
`unit_source` 区分 `file/user/unknown`。

可选 `schema` 为有序原生 TableSchema，包含 typed ColumnSchema ID、逻辑类型、
nullable 和可选 UnitSpec；也可用 `schema_path` 明确指定 v1 PlotiumDelimitedSchema
JSON 文件。声明必须与表头及顺序完全对应，非法值明确失败，不回退自动推断。
支持声明 null、boolean、int64、float64、utf8、date 和 timestamp；本导入不接受
唯一性/业务键约束及不确定度关系，也不从 `_sigma` 列名赋予科研误差含义。每次
导入分配新的原生 ID，文件/用户 schema 中的 ID 仅描述来源，不寻址已有项目目标。

每批最多 16 个文件、每文件 16 MiB、指定输入合计 64 MiB，每文件最多 256 列、
1000000 行；明确指定的元数据文件最多 256 KiB。已准备项摘要上限 128 KiB，
完整预检响应上限 256 KiB，超出时缩小批次或 schema。每连接最多保留 4 个已准备导入
和总计 32 个计划，同时只运行一个导入预检。这些是输入预算，不是进程总内存
上限。取消、切换项目、撤销权限或期间发生编辑，都会阻止待完成预检成为可执行
计划。排队或运行不代表完成；失败或过期提交也会消费计划，须重新读状态并预检，
不能盲目重放创建请求。

### 有界分页读完整张表

从导入元数据或数据集资源描述符复制 DatasetId 和表 RevisionId。将它们与
`expected_revision` 传给 `plotium_table_columns` 查看元数据，再调用
`plotium_table_read_rows`：

```json
{
  "expected_revision": 8,
  "dataset_id": "DATASET_UUID",
  "table_revision": "TABLE_REVISION_UUID",
  "offset": 0,
  "limit": 200
}
```

用返回的 ID 替换占位符。可选 `column_ids` 按稳定 ColumnId 投影并保持所给顺序；
省略或空数组表示 schema 顺序的全部列。行按不可变原生 snapshot 顺序读取。
结果包含 `column_ids`、`rows: [{row_id, values}]`、`total_rows`、`requested_rows`、
`returned_rows`、`next_offset` 和 `end`。每个值以 `kind` 标明类型，null 无 value。
Int64、time、timestamp 和 duration 用十进制字符串避免 JSON 舍入；date 为 Unix
纪元以来天数，后三种时间类型为纳秒。非有限浮点值明确返回 `NaN/+Inf/-Inf`
字符串，不转为 null。

保持数据集、文档 revision、表 revision 和投影不变，沿 `next_offset` 连续读取，
直到 `end: true` 且 `next_offset: null`。默认每页 200 行，最多 1000 行和 20000 个
投影单元格；完整结构化 ToolResult 上限 256 KiB。字节预算可缩短整页，但不截断
单元格；以实际数量和继续位置为准。单行超预算返回 `response_too_large`。
offset 等于总量时返回空的结束页，大于总量时失败。非法 limit、重复/不存在列及
超预算请求直接拒绝，不静默裁剪参数。

每次读取都检查两个 revision。人工编辑、Undo/Redo 会使旧文档 revision 失效，
须重新读状态并重启分页，不能混读。无效 ID 不回退当前选择。只读连接可查询
已有表格，但不可导入。预算和导入权限条件可从 `connection_capabilities` 发现。

### 精确重绑已有曲线，不影响另一张图

两个图可以引用同一张原生表，各自选择不同的数值 X、Y 和可选对称 Y 误差列。
在 GUI 中选中图对象，展开其 Series 的 **Data → Curve columns**，选列后点击
**Apply curve binding**。表格共享的默认绑定不会改变。

MCP 先通过 `plotium_document_inspect` 取得完整 Series target，再从
`plotium_table_columns` 复制 DatasetId、TableId、FieldId、表 revision 和 ColumnId。
使用项目 ColumnId，不使用 `source_column_id`、名称或显示 trace 的位置，也不猜字段
资源地址。调用 `plotium_plan`：

```json
{
  "expected_revision": 8,
  "tool_id": "plot.series.bind",
  "parameters": {
    "edits": [{
      "target": {
        "resource": {"id":"CANVAS_UUID/OBJECT_ID","kind":"plotium.canvas.object",
          "parent_id":"CANVAS_UUID","local_id":"OBJECT_ID"},
        "component": {"kind":"series","id":0}
      },
      "source": {
        "dataset_id":"DATASET_UUID", "field_id":0,
        "table_id":"TABLE_UUID", "table_revision":"TABLE_REVISION_UUID",
        "x_column_id":"X_COLUMN_UUID", "y_column_id":"Y_COLUMN_UUID",
        "y_error_column_id":null
      }
    }]
  }
}
```

将占位符和示例中的数值 ID 全部换成实际发现结果。不要另传顶层 `ids` 或
`targets`，每项编辑已携带完整目标。`y_error_column_id` 为 ColumnId，或明确的
null，后者移除误差绑定。检查 `preflight.binding_plan.bindings` 中的来源映射和
逐行诊断，再以返回的一次性 `plan_id` 调用 `plotium_execute`。

一个默认表格 Series 可能代表多条显示曲线，须先调用
`plot.series.materialize_default`，参数为
`{"edits":[{"target":EXACT_SERIES_TARGET,
"retain_y_column_id":"EXISTING_Y_COLUMN_UUID",
"table_revision":"TABLE_REVISION_UUID"}]}`。
GUI 对应 **Expand default curves**：选择保留原 SeriesId 的 Y 列，其余已有曲线
获得新的对象内 SeriesId，预检和执行结果均返回映射。此操作只展开已有内容，不创建
图对象。拟合叠加及归一化／偏移的多曲线默认组合无法无损展开，因而明确拒绝；
未展开的旧默认绘图入口仍可使用。

只接受 Int64 和 Float64 列；无法在 f64 图坐标中精确表达的整数明确拒绝。
X/Y 为 null 或非有限值时形成断点，不补零、不跨断点连线。误差为 null 时保留
有效点但不画该点误差线；零误差合法。负值或非有限误差使整批失败，即使该行的
X/Y 缺失也不忽略非法误差。误差表示非负绝对幅度，不从列名赋予标准差、标准误或
置信区间含义。诊断统计全部行，每条曲线最多附 20 个受影响 RowId；绑定校验和
原生曲线提取不抽样行。

兼容单位换算到图对象持久化的坐标单位；误差幅度仅使用比例，不应用仿射偏移。
已知单位与未知单位不能混用。重绑、删除首条曲线和重排后，手动范围及位移保持
原坐标含义；不兼容时拒绝，而不是重新解释原数字。人工标签、颜色、线／散点编码、
可见性、面板位置及轴设置随原目标保留。显式图保持线／散点编码，不切换到无关
表格图型。

每批允许 1–200 项编辑，受影响图合计最多 2,000,000 次曲线行访问；待执行 MCP
曲线计划共享同样的行预算，完整响应最多 256 KiB。全部目标、列、单位、行和
revision 检查通过后才一次原子提交、一次 Undo，不部分成功、不跳过无效目标。
只读连接拒绝绑定预检和执行。失败或过期执行也消费计划，须重读状态并重新规划。
GUI 保存重开保留原生绑定和单位；这些操作不开放 MCP 保存、导出或通用建图。

### 读取实际画布图像

可以请 agent：“预览这张画布，描述文字、曲线和本地图片。”先通过
`plotium_document_inspect` 取得完整画布 target，再以当前 revision 调用
`plotium_render_preview`：

```json
{
  "expected_revision": 7,
  "target": {"resource":{"id":"CANVAS_UUID","kind":"plotium.canvas"}},
  "max_edge": 1440
}
```

将 `CANVAS_UUID` 换成发现返回的画布 ID；不接受面板、内容、当前选择或 Series。
结果包含标准 MCP PNG **image 内容块**，以及 `ToolResult.value` 中的
`target`、`render_revision`、`width_px`、`height_px` 和 `mime_type`。
无需打开文件或自行渲染 SVG 文本。MCP 客户端须将图像内容交给 agent；纯文本
客户端无法读取此预览。

图像覆盖整页，使用原生版式、背景、面板裁剪、文字、曲线及项目内嵌栅格资产。
渲染基于不可变快照在后台执行，`render_revision` 标识该快照；截取后发生的人工
修改不会被标成图像内容。执行后续修改前请重新读取状态。只读连接也允许预览。
完成的读取保存仅含元数据的运行记录，使项目变为未保存，但不推进 revision。

`max_edge` 默认 1440 px，可取 1–2048；保持宽高比，按像素取整，每轴至少一个
像素。预算为输出 4,194,304 像素、引用资产编码数据合计 16 MiB、按实际文件头
探测的资产页 RGBA8 合计 64 MiB、中间 SVG 8 MiB、PNG 4 MiB、JSON-RPC 预览响应
6 MiB（包含请求 ID）。同一资产编码数据只计一次，同一资产页解码只计一次。
这些限制约束负载和图像缓冲区，不代表整个进程的内存上限。所有引用资产均校验，包括隐藏项。
发现信息 `connection_capabilities.limits.image_preview` 也提供这些预算。

资产缺失、损坏或图片页不可用时返回 `preview_asset_unavailable`，不会以占位图
或缺图结果报告成功。超过预算返回 `preview_budget_exceeded`，不静默降质；输出
超限时降低 `max_edge`，资产超限时缩小源资产。无效完整身份返回
`invalid_parameters`，过期 revision 返回 `stale_revision`。
每连接最多同时渲染一张预览，重复请求返回 `busy`；渲染期间其他已有工具仍可用。
取消请求或停止连接后不交付图像；停止连接、切换项目会丢弃旧任务。

### 查询并修改一条明确曲线

示例任务：“找到我的线图，读取线宽，预检改为 1.5 pt，再执行保存的计划。”
工具调用顺序如下：

1. 用 `{}` 调用 `plotium_get_state`，取得 `revision`、项目概况、选择和连接权限。
2. `plotium_resources_search` 接受 `expected_revision` 和 `query`，例如
   `{"kinds":["plotium.canvas.object"],"name_contains":"Plot"}`。先将结果解析为
   显式稳定 ID；`plotium_resources_inspect` 用 `ids` 和 `expected_revision` 读取描述。
3. 用 `{}` 调用 `plotium_properties_list` 列出支持的属性及范围。
   使用 `plotium_properties_inspect` 查询曲线地址：

   ```json
   {"ids":["CANVAS_UUID/OBJECT_ID"],"key":"series.line.stroke_width","expected_revision":7}
   ```

   返回的 `value.readings` 为每条适用曲线提供一行。复制所选曲线的完整
   `value.readings[0].target`，例如：

   ```json
   {
     "resource": {
       "id":"CANVAS_UUID/OBJECT_ID",
       "kind":"plotium.canvas.object",
       "parent_id":"CANVAS_UUID",
       "local_id":"OBJECT_ID"
     },
     "component":{"kind":"series","id":0}
   }
   ```

4. 将该 target 原样传入 `plotium_plan`，仅修改这一条曲线：

   ```json
   {
     "tool_id":"properties.set",
     "parameters":{"key":"series.line.stroke_width","value":1.5},
     "targets":[{
       "resource":{
         "id":"CANVAS_UUID/OBJECT_ID",
         "kind":"plotium.canvas.object",
         "parent_id":"CANVAS_UUID",
         "local_id":"OBJECT_ID"
       },
       "component":{"kind":"series","id":0}
     }],
     "expected_revision":7
   }
   ```

5. 查看 `preflight` 后，仅以 `{"plan_id":"RETURNED_PLAN_ID"}` 调用
   `plotium_execute`。桌面立即显示修改；**Undo** 恢复原值，**Redo** 重新应用。
   相同值报告为跳过；整批修改合并为一次撤销。

属性查询和属性编辑预检必须且只能提供一个非空的 `ids` 或 `targets` 数组。
`ids` 将每个资源展开为该属性对应的所有组件；`targets` 只操作指定的 Series 组件。
Series ID 仅在所属图对象内唯一，因此务必复制包含资源信息的完整 target。
重复目标仅处理一次；组件不存在或资源身份被伪造时整次请求拒绝；曲线存在但属性
不适用时报告为跳过。查询会返回当前值、默认值、范围及可用性；只查询一条曲线时，
也可将查询参数改为 `targets`。恢复默认值时，用同一 target 预检 `properties.reset`，
参数为 `{"key":"series.line.stroke_width"}`。组件寻址仅支持所列五项显示属性，
不开放处理参数。`resource.rename` 仍须使用 `ids`。

工具结果同时提供结构化内容及 JSON 文本。项目读取与预检要求当前
`expected_revision`；修改或撤销后重新获取状态。执行时重新检查保存的 revision
和当前权限。计划仅可提交一次，失败后也需重新预检；最多保留 32 个待执行计划，
重启连接会清空计划。桌面开始执行前已取消的请求不会修改项目；已进入原子
提交的修改无法通过取消回滚，请使用 **Undo**。

稳定错误码包括 `permission_denied`、`expired_plan`、`stale_revision`、`busy`、
`invalid_parameters`、`unsupported_operation`、`plan_limit`、
`request_cancelled`、`connection_stopped`、`response_too_large` 和 `execution_failed`。`busy` 可在编辑手势、
恢复确认或后台项目操作结束后重试；过期计划及 revision 冲突需重新读取和预检。
Agent connections 设置页面显示最近操作和错误，后台反馈不会自行打开设置页面。
