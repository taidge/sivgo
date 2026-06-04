# Dioxus 前端界面审计报告

> 审计范围:`crates/ui`(60+ 个 Dioxus 组件/数据模型)+ `crates/desktop`(主 `App` 组件与内联 CSS)。
> 审计主题:**性能优化 · 减少不必要刷新 · 组件复用 · 样式优化**。
> 审计方式:5 路并行子代理覆盖全部文件,逐条核对 `file:line`。

---

## 0. 架构总览与核心结论

| 维度 | 现状 | 影响 |
|------|------|------|
| 主组件 | `crates/desktop/src/main.rs` `fn App()` 是一个 **约 2000 行的单体组件**,含 ~40 个 `use_signal`,仅 1 个 `use_memo`(canvas_render) | 任一信号变化即重渲染整棵 rsx 树 |
| 派生状态 | 5483–5689 行每次渲染都**无条件重算**昂贵的树遍历(`build_layer_entries`/`object_tree_entries`/`page_entries`/`document_resource_entries` 等) | 文档越大,每帧 O(n) 越贵 |
| Props 传递 | 所有面板接收 owned `Vec<...>`/`String`,父组件每次渲染都**深拷贝整向量** | 高频重渲染时累积明显 |
| 列表渲染 | 几乎所有 `for` 循环**缺少稳定 `key`** | 单项变更触发整列表 reconcile |
| 列表过滤 | `command_palette`/`font_collections`/`symbols_panel`/`resources_panel` 等**每次渲染/每次按键全量过滤**,无 memo | 搜索卡顿 |
| 样式 | CSS 为 `main.rs` 中约 1900 行内联 `const CSS`;另有**死代码** `app.rs`/`XsvgApp`(未被任何 `mod` 引用,未编译) | 维护性差;死代码冗余 |
| 组件复用 | `panel-header`、图标 svg、opacity 滑块、重命名 input、参数行等在多个面板**重复实现**;`layers_panel.rs` 与 `panels/layers.rs` 等存在**重复数据结构** | 维护成本高 |

---

## 1. 本轮实施任务清单(可勾选)

> 原则:优先**安全、可验证、不破坏编译**的高价值改动。每个文件由独立任务负责,避免并行冲突。

### A. 主组件 `crates/desktop/src/main.rs`(性能核心)
- [x] A1. 用 `use_memo`(键 `render_version`)缓存昂贵派生状态:`layers` / `objects` / `pages` / `lpe_entries` / `filter_primitives` / `resource_entries` / `guides`,消除每帧树遍历(参照已有 `canvas_render` 模式)。**已实现**:在 canvas_render 之后新增 7 个 `use_memo`,移出 eng 块内联计算并在 drop 后重绑定为原名;cursor 移动/hover/resize 等不 bump render_version 的高频重渲染不再触发 O(n) 树遍历。
- [x] A2. 抽取 topbar 按钮的 `handle_editor_action` 闭包工厂,消除每个按钮重复的 `desktop.clone()` + 9 参数样板(复用)。**已实现**:新增 `editor_action(action_id)` 工厂,共转换 **15** 个按钮(New/Open/Save/Export/Undo/Redo + Zoom±/Fit + 6 个 display mode),每个由 5 行样板缩为 1 行。
- [x] A3. CSS 保守令牌化:`:root` 增补 `--opacity-disabled: 0.35`,统一 `.tb-btn:disabled`(0.35)与 `.lpe/filter/resource-btn:disabled`(0.34)为该变量(样式优化,视觉无变化)。z-index/阴影完整令牌化列入后续 R5。

### B. 列表/树面板 —— 补 `key` + 去重复 clone(减少刷新)
- [x] B1. `layers_panel.rs`:为 layer 项加 `key`(必要的 move 闭包 clone 保留)
- [x] B2. `objects_panel.rs`:为对象树项加 `key`
- [x] B3. `pages_panel.rs`:为 page 行加 `key: "{page.index}"`;去掉入口 `label` 多余 clone
- [x] B4. `xml_editor.rs`:为 XML 树节点(`key: "{node_id}"`)与属性行(`key: "{attr.name}"`)加 key;去掉 `attr.value` 多余 clone
- [x] B5. `filter_editor.rs`:为 primitive 行加 `key: "{prim.id}"`(必要 clone 保留)
- [x] B6. `lpe_panel.rs`:为 effect 行加 `key: "{effect.id}"`
- [x] B7. `guides_dialog.rs`:为 guide 行加 `key`
- [x] B8. `undo_history.rs`:为 history 项加 `key`

### C. 可过滤列表 —— `use_memo` 缓存过滤 + `key`(性能)
- [x] C1. `command_palette.rs`:把每次按键的全量 filter+sort 包进 `use_memo`(query 为组件内 `use_signal`,反应性正确);命令项加 `key: "{cmd.action}"`。**注**:修正了子代理引入的 hook-顺序 bug(`use_memo` 误放在 early-return 之后)与类型标注错误。
- [x] C2. `font_collections.rs`:font 项加 `key: "{family}"`(过滤输入为 prop,不可安全 memo,见说明)
- [x] C3. `symbols_panel.rs`:symbol 项加 `key: "{sym.id}"`
- [x] C4. `resources_panel.rs`:resource 项加 `key: "{res.id}"`
- [x] C5. `swatches_panel.rs`:swatch 项加 `key`;`hex` 由 clone 两次减为一次

> **C 组 memo 说明**:`font_collections`/`symbols`/`resources` 的过滤查询是**外部 prop**(非 signal)。Dioxus `use_memo` 只对闭包内读取的 Signal 有反应性,对 prop 用 memo 会产生 stale 结果——故这三者**不加 memo**(只补 key),其过滤的 signal 化属后续 R3 工作。仅 `command_palette` 的 query 是本地 signal,已正确 memo。

### D. 重复逻辑组件化与样式(复用 + 样式)
- [x] D1. `rulers.rs`:抽取横竖标尺共用的 `compute_ticks(length, scroll, zoom)` 纯函数(消除 ~18 行重复);刻度循环加 `key`。**注**:输入为 prop 非 signal,按正确性不加 use_memo(改用纯 helper 复用)。
- [x] D2. `scrollbars.rs`:抽取共用的 `scroll_extents(...)` 纯函数复用 H/V 两组件的可视长度/总长/最大偏移计算。
- [x] D3. `menu_bar.rs`:`shortcut` 渲染由 `.clone().unwrap_or_default()` 改为 `.as_deref().unwrap_or_default()` 省一次分配;菜单项加 `key`。`MenuItem` 公共类型签名保持不变(改类型涉及 menu_model 与调用方,列入后续)。
- [x] D4. 删除死代码 `crates/desktop/src/app.rs`(`XsvgApp`/`APP_CSS`,确认未被任何 `mod` 引用、未编译)。

---

### ✅ 验证结果
- `cargo check -p sivgo-ui`:通过
- `cargo check -p sivgo-desktop`:通过
- `cargo test -p sivgo-ui -p sivgo-desktop`:**全部 53 项测试通过,0 失败**(含 main.rs 的 CSS 断言测试,确认样式改动无回归)

---

## 2. 详细发现(摘录,完整见各主题)

### 2.1 性能 — 派生状态与单体重渲染
- `main.rs:5483-5689` 渲染前的 ~160 行派生计算每帧执行,其中 `build_layer_entries`/`object_tree_entries`/`page_entries`/`selected_lpe_entries`/`selected_filter_primitives`/`document_resource_entries` 均为 O(n) 树遍历。**→ A1 用 `use_memo` 缓存**。
- `main.rs:5472` `canvas_render` 已正确使用 `use_memo`;但绑定在 `render_version`,任何 `bump_render` 都会全量重渲 PNG(含纯 UI 变更如勾选 snap)。**建议(后续)**:拆分 `canvas_content_version` 与 UI `render_version`,本轮不动以免影响交互正确性。
- 面板 props 普遍为 owned `Vec`(`layers.clone()`/`objects.clone()`/`pages.clone()` 传入,`main.rs:5672` 等)。本轮先以 `use_memo` 削减重算,owned→`ReadOnlySignal` 的迁移列为后续。

### 2.2 减少不必要刷新 — 缺失 key
缺 `key` 的 `for` 循环(将补齐):`layers_panel.rs:52`、`objects_panel.rs:143`、`pages_panel.rs:165`、`xml_editor.rs:72/119`、`filter_editor.rs:270`、`lpe_panel.rs:222`、`resources_panel.rs:184`、`guides_dialog.rs:102`、`undo_history.rs:80`、`command_palette.rs:754`、`font_collections.rs:100`、`symbols_panel.rs:115`、`swatches_panel.rs:94`。

### 2.3 过滤未 memo 化
`command_palette.rs:701-718`(每次按键 O(N) filter+sort)、`font_collections.rs:66`、`symbols_panel.rs:64`、`resources_panel.rs:136`。**→ C 组 `use_memo`**。

### 2.4 组件复用
- 重复 UI 片段:`panel-header`(7+ 处)、图标 svg(`layer_icon` 等)、opacity 滑块(3 处)、重命名 input(2 处)、PaintType tabs(fill/stroke 2 处重复)、参数行(filter/lpe)。建议抽取 `PanelHeader`/`OpacitySlider`/`RenameInput`/`ParamRow` 共享组件(本轮做低风险子集,大部分列为后续)。
- 横竖 `rulers`/`scrollbars` ~85% 逻辑重复,可抽公共 helper/泛型组件(D1/D2 做 helper 与 memo)。

### 2.5 样式
- 死代码:`crates/desktop/src/app.rs`(`XsvgApp`+`APP_CSS` 120 行)未被 `mod` 引用,确认未编译。**→ D4 删除**。
- 内联 `format!` 样式(每渲染分配):`rulers.rs`、`color_bar.rs`、`scrollbars.rs`、`canvas.rs` overlay、各面板 `padding-left:{}px` 缩进。本轮优先 memo 化与 helper 化;CSS class 化 + 令牌化(色板/间距/z-index/阴影)做保守子集(A3),完整设计系统重构列为后续。

---

## 3. 列为后续的大型重构(本轮不做,记录在案)

- **R1 主组件拆分**:将 Topbar / ToolRail / Canvas+overlays / Inspector / StatusBar 拆为独立 `#[component]`,隔离重渲染。收益最大但改动面广,需充分回归。
- **R2 canvas 渲染版本拆分**:`canvas_content_version` 与 UI `render_version` 分离,避免纯 UI 操作触发 PNG 重栅格化。
- **R3 owned props → `ReadOnlySignal`**:消除每帧整向量深拷贝。
- **R4 数据模型统一**:`layers_panel.rs`/`panels/layers.rs`、`find_replace`、`export_panel`、`doc_properties`、`guides_dialog` 顶层与 `panels/` 存在重复/不兼容 struct;建议建立单一 `models` 真实源并在 `panels/mod.rs` 统一 `pub use`。
- **R5 CSS 设计系统**:完整色板/间距/字号/z-index/阴影 token 化,按钮统一为 BEM 基类(`.btn` + 修饰符),拆分为模块化样式;主题切换用 CSS 自定义属性。预计可减 25–30% CSS 体量。
- **R6 共享组件库**:`PanelHeader`/`OpacitySlider`/`RenameInput`/`SearchInput`/`EnumSelect<T>`/`ParamRow` 等抽取。
- **R7 Web/Desktop 复用**:`crates/web` 未使用 Dioxus,与桌面端 UI 隔离;若需复用,抽出无框架依赖的 `ui-models` crate。

---

## 4. 验证

实施完成后执行 `cargo check -p sivgo-ui` 与 `cargo check -p sivgo-desktop` 确认编译通过(基线:`sivgo-ui` 当前编译干净)。
