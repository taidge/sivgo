# Sivgo 代码评审报告与整改记录 `_improve.md`

> 评审日期：2026-09-02
> 评审范围：整个 workspace，14 个 crate、约 11.3 万行。
> 评审方式：7 名评审员按 crate 分组逐文件通读（非抽样），主评审对关键结论逐条复核，其中 5 条通过**实际运行程序**验证。
> 整改状态：报告的**三批建议已全部实施**，另加一轮激进改造，处理了"写了没接入"的绝大部分条目。逐条状态见 `[x]` / `[ ]`。
> 前一轮报告可通过 `git log -p -- _improve.md` 取回。

---

## 0. 验证基线

| 检查 | 整改前 | 整改后 |
|---|---|---|
| `cargo build --workspace` | 通过 | 通过 |
| `cargo test --workspace` | 1640 通过、0 失败 | **1785 通过、0 失败**（新增 145 项） |
| `cargo clippy --workspace --all-targets -- -D warnings` | **失败**（Rust 1.98 新增 lint） | **通过，零警告** |
| `cargo fmt --all -- --check` | 通过 | 通过 |
| `RUSTDOCFLAGS=-D warnings cargo doc` | 通过 | 通过 |
| 崩溃复现用例 | 8 例全部中止进程或 panic | **10 例全部正常返回** |
| 代码净变化 | — | 104 文件，+5405 / −3367 行 |

**关于"零警告零失败"的解读**：整改前所有严重缺陷都落在测试与 lint 的覆盖盲区——`crates/web` 没有任何 HTTP 层集成测试，渲染测试只断言"画面非空且两次渲染一致"，序列化没有往返测试，变换测试只用平移（平移可交换，掩盖了顺序错误），CRDT 测试只做两副本单向合并。本轮补齐的正是这几类测试，它们在编写过程中直接发现了**四个原报告未列出的缺陷**：序列化不幂等、clipPath 互引用栈溢出、按键表示不匹配、`view.zoom_fit_page` 未登记。

---

## 一、P0：会直接毁数据或打死进程（全部已修复）

### D1 保存带文本或 LPE 的文档后，程序自己打不开该文件

同一属性被写出两次，产生非法 XML。整改前实测两例均 `reparse=FAIL`。

- [x] 序列化层引入去重写入器，覆盖全部 248 处属性写入。
- [x] `path-effect` / `original-d` 加入 `handled_attrs`，消除重复存储。
- [x] 新增 [roundtrip.rs](crates/dom/tests/roundtrip.rs)，覆盖文本、LPE、渐变、全部 examples 文件。

**新发现并修复**：序列化不幂等——文本节点会吸收缩进空白，每存一次盘就多一层。已按 `xml:space="default"` 规范化。

### D2 引用环与深层嵌套导致栈溢出，进程 abort

- [x] 渲染器加 RAII 深度守卫，覆盖 `<use>`、pattern、mask、marker 的直接与间接递归。
- [x] **新发现**：clipPath 互相引用不经过 `render_node`，单独加守卫。
- [x] 解析改为迭代遍历，并在解析前对原始 XML 做深度预扫描。上游 XML 库自身约 100 层就溢出，无法事后捕获，因此深文档改在 128 MiB 栈的线程上解析。

### D3 空 `<feMerge/>` 触发断言 panic

- [x] 流水线两端统一归一缓冲尺寸，保护所有原语。
- [x] 顺带修 `feMorphology` 的 `i32` 溢出与 `feTurbulence` 的 octaves 无上限。

### D4 `Color::from_hex` 对多字节字符 panic

- [x] 入口拒绝非 ASCII。

### D5 `RemoveNodeCommand` 撤销后子孙节点永久丢失

- [x] 改为快照整棵子树，undo 时还原父子链与兄弟位置。

---

## 二、P1：核心语义错误（全部已修复）

### R1 嵌套 transform 复合顺序反了

整改前实测输出与"错误预期"逐字节相同。

- [x] 节点嵌套、`<use>` 的 x/y、`<symbol>` viewBox 三处顺序更正。
- [x] `parse_transform` 属性列表顺序更正（原单测把错误语义固化了）。
- [x] 新增 [transform_semantics.rs](crates/render/tests/transform_semantics.rs)，把嵌套结构与等价展平结构逐像素比对，每个用例都混入缩放或旋转。

### R2 引擎变换围绕错误中心

整改前翻转把对象推到 (-75, 40)。

- [x] 五处中心化变换更正，旋转改用 `rotate_about`。
- [x] 新增 [object_transforms.rs](crates/core/tests/object_transforms.rs)，断言中心不变、四分之一转宽高互换、撤销可回原位。

### R3 `node_transform` 祖先链顺序颠倒

- [x] fold 方向更正，附祖先链与命中测试断言。

### R4 `feComposite` 的 `atop` 公式错误

- [x] 第一项改用源 alpha，附 3 项测试。

### R7 CSS 完全没有接入 — **已完整接入**

此前 Inkscape 输出的 `style="fill:..."` 全部渲染成黑色。

- [x] 解析层合并内联 `style` 声明，优先级高于同名属性。
- [x] 渲染层实现属性继承，可继承集合与 `sivgo_style::is_inherited` 对齐。
- [x] **`<style>` 元素与 class/id/后代选择器已接入**：新增 [overrides.rs](crates/style/src/overrides.rs) 把规则解析成按节点的覆盖表，渲染器查表。CSS 优先级完整：属性 < 样式表规则 < 内联 style。
- [x] 修正 style crate 中 `font-size` 的 `em` 应参照父级而非累积值。
- [x] 新增 [style_resolution.rs](crates/render/tests/style_resolution.rs)，34 项测试。
- 实测 Inkscape 风格文件配色由全黑变为正确。

### R8 其余渲染语义缺失 — **已全部修复**

- [x] `fill-opacity` / `stroke-opacity`：`RenderPaint::with_opacity` 对纯色、渐变、图案分别缩放。
- [x] `<rect>` 的 `rx`/`ry` 圆角，按规范镜像缺省半径并钳制。
- [x] `stroke-dasharray` 奇数值按规范复制一遍。
- [x] 百分比 / em / ex 单位：新增 `RenderContext` 承载视口与字号，按轴分别解析，`viewBox` 会重定义参照。
- [x] `SvgNode.hidden` 现在会被渲染器读取，隐藏图层对导出生效。
- [x] `requiredFeatures` / `requiredExtensions` 改从类型化字段读取，`<switch>` 不再永远取第一个分支。
- [x] 离屏栈不平衡：Defs 与 Use 的提前返回路径补上隔离层弹出。
- [x] **未知元素不再被改写成 `<g>`**：新增 `SvgElement::Unknown` 保留原标签、命名空间前缀与文本内容。实测 `<metadata>` 与 `<inkscape:path-effect>` 完整往返。同时按规范不渲染 `<foreignObject>`、`<script>`、字体等非图形元素的内容。

---

## 三、P1：Web 后端（全部已修复）

- [x] **W1** 路由参数语法 `<id>` 改为 `{id}`。此前文档读取、资产下载、分享链接、OAuth 登录、插件安装全部 404。新增 [routes.rs](crates/web/tests/routes.rs)，21 项集成测试。
- [x] **W2** 请求体真实上限 64 KiB：挂传输层大小中间件与 `SecureMaxSize`，业务层的检查现在真正可达。
- [x] **W3** 公开导出可读任意本地文件：新增 `render_document_to_pixmap_without_file_access`，`<image href>` 只接受 `data:`。
- [x] **W4** OAuth 按 email 自动关联账号：改为按 `(provider, subject)` 身份绑定；地址已被占用时返回冲突，要求从已认证会话显式关联。
- [x] **W5** 限流绕过：`X-Forwarded-For` 改取最右值；认证路由挂按用户限流；限流表加容量上限并只在超限时清理；每账户文档数与每文档版本数配额；删除文档级联清理。
- [x] **W6** 无超时无兜底：挂 `CatchPanic` 与 30 秒超时。
- [x] **W7** `reqwest::blocking` 在异步上下文：改为异步并设 10 秒超时（路由测试一写出来就暴露了这条）。
- [x] **W8** 登录 dummy verify 方向反了（改为预计算的静态哈希）；配置结构体的 `Debug` 会泄露密钥（改为手写并打码）；`RwLock` 中毒后永久 500（27 处改为容忍中毒）。

---

## 四、写了但没接入 — **本轮激进处理**

原报告最大的一块。本轮的方针是逐项决定接线还是删除，不再积累。

### 已删除（名不副实或已被取代）

- [x] **render 的 vello / canvas2d / webgpu 三个"后端"**（1470 行）：实为 tiny-skia 重放的包装，零引用，README 已更正描述。
- [x] **io 的 pdf_export.rs**（712 行）：孤儿文件，其 `dpi` 选项只写进元数据、并不真正缩放，价值不如现用版本。

### 已接线

- [x] **io 的 dxf_export / eps_export**（1089 行）：这两个孤儿版本比现用版本更完整（带选项、Y 轴翻转、文件写入），**替换**了原实现。
- [x] **三个导出器共 49 处直接读 `.value` 忽略单位**：统一改为 `to_px()`。新增 [export_formats.rs](crates/io/tests/export_formats.rs)，12 项测试覆盖结构、毫米换算与 Y 轴翻转。
- [x] **`crates/style`**：见 R7，现已被渲染器实际使用。
- [x] **`builtin_markers`**：从 render 移到 core（它是文档内容不是渲染逻辑），标记工具现在可放置全部 9 种内置标记，而非硬编码单一箭头。
- [x] **桌面端快捷键**：`ShortcutManager` 接进画布键盘事件，菜单项注册真正的加速键。**新发现**：绑定表用 `Key::S` 这类具名变体，而我的转换初版产出 `Char('s')`，测试立刻抓到。同时补齐 16 种加速键符号解析。
- [x] **工具参数**：`Tool` trait 加 `as_any_mut` / `as_any`（24 个工具全部实现），`ToolManager` 提供类型化访问。工具选项栏的 X/Y/W/H、圆角、星形角数与比例、螺旋圈数、铅笔平滑、字体族/字号/粗体/斜体全部接通，此前是空操作。
- [x] **LPE 列表对齐**：UI 的 `LpeKind` 从 10 项固定枚举改为携带引擎规范 key，面板现在提供全部 26 种效果。此前 18 种既不能添加也看不见、删不掉。
- [x] **io 的公共 API**：Import 菜单接通（SVG 合并、位图嵌入、位图描摹），"Export Plain SVG" 改为真正调用 `export_plain_svg`（此前走普通序列化，并不剥离 inkscape 属性）。引擎新增 `import_document` 作为单步可撤销命令。
- [x] **`crates/core/src/collab/`**：修正跨站点因果丢失导致的**副本永久分歧**缺陷（Update 的目标未到达时被静默丢弃，现改为缓冲重试），并加上 serde 使操作可跨网络。新增 [crdt_convergence.rs](crates/core/tests/crdt_convergence.rs)，6 项测试覆盖三站点、任意投递顺序、幂等与序列化往返。已验证：去掉修复后其中 2 项失败。

### 仍未接线（需要产品决策或属独立特性）

- [ ] `DamageTracker` / `TileGrid` / `RenderCache`：增量渲染需要配合画布管线改造。
- [ ] collab 的网络层：CRDT 现在正确且可序列化，但 WebSocket 房间、鉴权与引擎集成是独立的产品特性。
- [ ] 插件系统：两个"运行时"都不成立（无 wasmtime，无 Lua），`PluginAction` 无执行方。这一块需要先决定是引入真实运行时还是下架。
- [ ] 最近文件列表、`file.new_from_template`、Extensions 菜单等 30 余个菜单项。

---

## 五、工程与文档

- [x] **E1** clippy 在 Rust 1.98 下失败：修复两类新 lint，现零警告。
- [x] **E2** README 夸大：更正渲染后端、插件运行时、CSS、io 导入导出四处表述与依赖图。
- [x] **E3** CI：加最小权限声明；新增 Windows/macOS 检查（桌面端依赖平台代码，此前从未在这两个平台构建）；新增 MSRV 任务；新增 fuzz 构建与短时运行。
- [x] **E4** 12 处未使用依赖已移除。
- [x] fuzz 覆盖：新增渲染与 PDF 导入两个目标（此前只有 SVG 解析且从未在 CI 跑过）。
- [x] 新增 [malformed_input.rs](crates/render/tests/malformed_input.rs)，6 组约 45 个畸形输入用例，常规 CI 即可运行，无需 nightly。
- [ ] Actions 未 SHA 锁定；无 Dependabot / cargo-deny；RustSec 例外在两处重复。

---

## 六、本轮验证证据

| 验证项 | 方式 | 结果 |
|---|---|---|
| 存盘后可重新打开 | 独立程序跑 `parse → serialize → parse` | LPE 与文本两例由 FAIL 转为 OK |
| 序列化幂等 | 连续两次序列化比对 | 由不等转为相等 |
| 元数据与 LPE 定义保真 | 往返比对 | `<metadata>`、`<inkscape:path-effect>` 由被改写成 `<g>` 转为完整保留 |
| 8 种引用环 + 深层嵌套 | CLI 渲染 | 10 例由中止进程转为正常返回 |
| 嵌套变换顺序 | 渲染后与展平形式比对 md5 | 由匹配"错误预期"转为匹配正确形式 |
| 形态学性能 | 三种半径计时 | 32.7 秒 → 1.2 秒，且与半径无关 |
| 形态学算法等价性 | 与朴素实现逐像素比对 | 2 种操作 × 7 种半径 × 2 个方向全部一致 |
| Inkscape 风格文件配色 | 渲染后读取像素 | 由全黑转为正确配色 |
| CRDT 收敛缺陷 | 临时移除修复后重跑 | 2 项测试失败，恢复后全通过 |
| 快捷键映射 | 遍历默认绑定表 | 每条绑定都落到真实处理器 |

---

## 七、剩余工作

**插件系统**是现在最大的一块未决项：manifest 与权限模型合理，但两个"运行时"都只是玩具（WASM 解释器只支持约 20 条指令，"Lua" 是字符串匹配的 DSL），`PluginAction` 没有执行方。需要先决定引入真实运行时还是下架，再谈接线。

**画布渲染管线**（全文档栅格化 → base64 → `<img>`）仍是最大的技术债，决定了性能上限与放大清晰度。视口与百分比的基础设施已经就位，改造成按视口栅格化的条件比之前好。

**协作网络层**现在只差传输：CRDT 收敛正确、可序列化，缺的是 WebSocket 房间、鉴权与引擎集成。

其余是长尾：最近文件、模板、Extensions 菜单，以及 CI 的供应链加固。
