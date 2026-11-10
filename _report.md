# Sivgo 项目完成度、缺陷与 Gaps 统一审计报告

审计日期：2026-07-15

审计范围：`D:\Works\taidge\sivgo` 当前工作区

审计基线：`main` 分支；开始审计时 Git 工作区干净

工具链：Rust/Cargo 1.97.0；workspace MSRV 声明为 Rust 1.89

## 0. 执行结论

Sivgo 已经是一个结构完整、覆盖面很广的 Rust SVG 编辑器技术原型，但还不是可直接发布的生产级 Inkscape 替代品。当前 workspace 有 14 个 crate、274 个 Rust 文件、约 101,940 行 Rust 代码；核心库、桌面 GUI/CLI 和 Web REST API 都能通过编译、测试、格式、Clippy 和严格 rustdoc 门禁。

本次审计后的估算：

| 范围 | 完成度 | 结论 |
|---|---:|---|
| 基础库与编辑引擎原型 | 70–80% | 能力广、单测强，仍有 SVG fidelity 与集成长尾 |
| 桌面端可用原型 | 55–65% | GUI/CLI、系统剪贴板和主要面板可用，单体 UI 与平台能力仍是风险 |
| Web 后端 API 原型 | 45–55% | 基础鉴权、授权、限流和 OpenAPI 已有，数据仍全部在内存中 |
| Web/WASM 前端 | 0–5% | 当前 worktree 没有真实 Web 前端入口 |
| 协作产品能力 | 20–30% | CRDT/OT/presence 等模型存在，缺真实网络与 UI 闭环 |
| 插件产品能力 | 30–40% | 权限、WASM/DSL 边界和内置插件存在，缺桌面入口及动作落地 |
| CI、供应链与发布就绪度 | 55–65% | 常规门禁已加强，仍缺像素基线、E2E、fuzz CI、SBOM 与打包发布 |

综合判断：技术原型约 65–70%；若目标是稳定、可持久化、可分发、可协作的生产产品，则约 40–45%。完成度是基于当前代码可达能力和验证证据的工程估算，不是按文件数量计算。

## 1. 本轮已完成的改进

- [x] 修复 Rust 1.97 新增 Clippy 规则导致的 `--all-targets -D warnings` 失败。
- [x] 将本地 `just clippy` 和 GitHub Actions 统一为 `cargo clippy --workspace --all-targets -- -D warnings`。
- [x] 首次执行严格 rustdoc，修复 18 条断链、歧义或私有项链接诊断。
- [x] 将 `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` 加入 CI。
- [x] 执行 RustSec 审计；把 10 个未处理漏洞降为 0 个未例外漏洞。
- [x] 定向升级 `aws-lc-sys`、`crossbeam-epoch`、`quinn-proto`、`rustls-webpki`、`anyhow`、`memmap2` 和多个 `rand` 版本等传递依赖。
- [x] 增加每周、依赖变更和手工触发的 `.github/workflows/security.yml`。
- [x] 为两个仅存在于 `wayland-scanner` 构建期路径、当前无兼容升级的 `quick-xml` 告警增加有理由的临时例外；例外集中在 `.cargo/audit.toml`。
- [x] 把原先不会被 workspace 执行的 `tests/visual` 框架接入 `sivgo-render` 集成测试。
- [x] 10 个视觉语料现在会逐个解析、渲染、检查非空画面和重复渲染确定性；比较框架的 4 个单测也随 workspace 执行。
- [x] 修复公开 API 限流无条件信任 `X-Forwarded-For`、可被客户端伪造绕过的问题。
- [x] 新增 `TRUST_PROXY_HEADERS` 显式开关；默认按真实 socket IP 限流，只允许受信任代理部署主动启用转发头。
- [x] 生产环境 JWT secret 增加至少 32 字节的启动校验及回归测试。
- [x] README 补充代理头与 JWT secret 部署要求。
- [x] 运行 DOM parse/serialize benchmark 并记录当前基线。
- [x] 重写本报告，删除旧报告中“GUI 仍只使用 stub 剪贴板”等已过期结论。

仍未完成的核心产品项：

- [ ] Web 数据库、迁移、备份和多实例一致性。
- [ ] 生产级分布式限流与持久审计管道。
- [ ] 真正的 Web/WASM 前端。
- [ ] 协作 WebSocket/API/存储/UI 闭环。
- [ ] 插件桌面入口、第三方安装/升级和 `PluginAction` 主流程落地。
- [ ] 跨平台参考图像像素回归、桌面 E2E 和 fuzz CI。
- [ ] 安装包、签名、发布流水线、变更日志、SBOM 和依赖许可证策略。

## 2. 验证证据

| 检查 | 结果 | 说明 |
|---|---|---|
| `cargo metadata --no-deps --format-version 1` | 通过 | 14 个 workspace member |
| `cargo build --workspace` | 通过 | 最终复验见本报告完成后的验证记录 |
| `cargo test --workspace` | 通过 | 1,643 个列出的测试；1,640 通过、3 个 doctest ignored |
| `cargo fmt --all -- --check` | 通过 | 全 workspace 格式门禁 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过 | 包括 lib/bin/test/bench target |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 通过 | 严格 API 文档门禁 |
| `cargo audit` | 通过 | 0 个未例外漏洞；2 个构建期例外；16 个 informational/yanked 警告 |
| `cargo bench -p sivgo-dom --bench parse_serialize` | 通过 | 1000 rect parse 约 2.118 ms/iter；serialize 约 456.614 µs/iter |
| 视觉 corpus 集成测试 | 通过 | 10/10 SVG fixture 解析、非空渲染、重复渲染一致 |
| fuzz | 未运行 | 本机有 nightly，但未安装 `cargo-fuzz`；不能勾选 |
| 桌面 GUI E2E | 未运行 | 当前没有自动化 GUI/E2E harness；不能用编译通过替代交互验证 |
| release build/package | 未运行 | 本轮未验证安装包、签名或最终 release artifact |

注意：当前视觉测试是“语料 smoke + 确定性检查”，尚不是跨提交的参考 PNG 像素回归，因此对应 gap 仍保持未勾选。

## 3. 当前项目可审核的维度

建议至少从以下 18 个维度持续审核：

1. 产品范围与文档一致性：README 声明、路线目标和当前可达能力是否一致。
2. 功能完成度：编辑、选择、图层、页面、LPE、文本、滤镜等是否形成用户闭环。
3. 架构与模块边界：crate 依赖、领域边界、状态管理和单体文件风险。
4. 构建与工具链：MSRV、跨平台、debug/release、文档构建和可复现性。
5. 代码质量：格式、Clippy、错误处理、panic/unwrap、重复逻辑和文档质量。
6. SVG 标准与渲染 fidelity：DOM、CSS、paint、filter、text、mask、clip、marker、pattern。
7. 桌面 UX 与平台集成：文件、剪贴板、拖放、自动保存、多文档、快捷键和设置。
8. Web/API 正确性：路由、错误模型、OpenAPI、鉴权、授权、并发和请求限制。
9. 数据持久化：数据库、迁移、事务、备份、恢复、版本历史和多租户隔离。
10. 安全：认证、secret、CORS、代理边界、限流、上传、插件沙箱和 DoS。
11. 供应链：RustSec、yanked/unmaintained crate、许可证、来源策略和 SBOM。
12. 测试策略：单元、集成、视觉、E2E、fuzz、benchmark、coverage 和跨平台矩阵。
13. 性能与容量：大 SVG、大图片、滤镜、文本、导出、内存上限和增量渲染。
14. 导入导出互操作：SVG/PNG/JPEG/PDF/EPS/DXF 等格式的真实 fidelity。
15. 插件扩展：权限、执行边界、安装、更新、UI、兼容策略和第三方开发体验。
16. 实时协作：CRDT/OT、presence、comments、冲突、网络传输、离线恢复和存储。
17. 可访问性与国际化：键盘导航、语义/ARIA、焦点、对比度、i18n、RTL/CJK。
18. 运维、合规与发布：日志、指标、健康检查、配置、许可证、打包、签名和发布策略。

## 4. 分维度审核

### 4.1 产品范围与文档一致性

已确认：

- Workspace 确实包含 README 所列的 14 个 crate。
- 当前产品形态是 Dioxus Desktop 应用 + Salvo REST API；README 已明确没有 Web/WASM client。
- 当前 Web 后端无需外部数据库，因为实现确实是内存存储，而不是因为持久化已经完成。
- Rust 版本说明与 `rust-version = "1.89"` 一致。

缺口：

- [ ] 明确 Web 产品路线：正式取消 Web 前端，或恢复/重建 Web/WASM 客户端。
- [ ] 建立“功能声明 -> UI 入口 -> 集成测试”的追踪矩阵，避免库中存在类型就被当成产品完成。

### 4.2 架构与可维护性

优势：

- math/dom/style/render/path/text/filters/trace/io/core/plugin/ui/desktop/web 分层整体合理。
- 101,940 行代码分布在 274 个 Rust 文件中，多数领域能力已经模块化。
- `core` 虽约 30K 行，但分散在 97 个文件中，优于把全部编辑逻辑集中在一个文件。

主要风险：

- `crates/desktop/src/main.rs` 约 7,668 行，`App()` 从约第 5,561 行开始，UI 状态、菜单分发、剪贴板、偏好和编辑动作仍高度耦合。
- Web 的 `Store`、`AssetStore`、`PreferencesStore`、`PluginRegistry` 和 `AuditLog` 分别维护内存状态，未来迁移数据库时容易出现多套仓储接口和事务边界。

- [ ] 拆分 desktop app state、command dispatch、menu actions、document I/O 和视图组件。
- [ ] 为 Web 定义 repository/service trait，再引入数据库实现，避免 handler 直接绑定 HashMap store。

### 4.3 构建、代码质量与 CI

本轮状态：

- [x] fmt、all-targets Clippy、测试和严格 rustdoc 全绿。
- [x] CI 与本地 `just clippy` 的 target 范围一致。
- [x] 源码中没有 `TODO`、`FIXME`、`todo!()` 或 `unimplemented!()` 标记。

风险：

- “没有 TODO 标记”不代表完成；多个实现通过注释声明 passthrough、skip、placeholder 或 restricted DSL。
- CI 只有 Ubuntu；桌面产品缺 Windows/macOS 编译矩阵。
- 当前使用 `stable`，未单独验证声明的 MSRV 1.89。

- [ ] 增加 Rust 1.89 MSRV job。
- [ ] 增加 Windows/macOS 至少 `cargo check` 的矩阵。
- [ ] 增加 release build/package smoke。

### 4.4 SVG、CSS 与渲染 fidelity

已有能力：

- 基本形状、path、gradient、pattern、clip、mask、marker、image、text 和大量 filter primitive 有实现和测试。
- `RenderBackend` 抽象、TinySkia CPU 输出、render cache、damage/tiling 等基础设施已经存在。
- 新视觉语料覆盖 basic shape、path、linear/radial gradient、opacity、clip、blur、text 和 transform。

明确 gap：

- `feComponentTransfer` 因 DOM 未解析 `feFunc*` 子节点，目前仍是 identity passthrough。
- diffuse/specular lighting 的 `light_color` 仍硬编码白色。
- 未知或未实现 filter primitive 会记录 warning 后跳过。
- CSS parser 会跳过不支持的 pseudo-class，尚无完整浏览器级 CSS/SVG 兼容矩阵。
- Vello/WebGPU/Canvas2D 当前主要记录命令并通过 TinySkia CPU replay；不是已接通的硬件 GPU/浏览器渲染器。

- [ ] 解析并执行 `feFuncR/G/B/A`。
- [ ] 支持 lighting-color 和剩余 filter fidelity。
- [ ] 建立基于 SVG 规范/W3C corpus 的兼容矩阵。
- [ ] 接入真实 Vello/WebGPU/Canvas2D adapter，并分别验证输出。

### 4.5 编辑器核心与桌面体验

已有能力：

- EditorEngine、命令、undo/redo、选择、snapping、图层、页面、文本、LPE 和大量工具均有测试。
- Desktop 提供 GUI，以及 `render`/`info` CLI。
- 系统剪贴板已经接入主流程；SVG、文本和 bitmap 会优先使用 `ClipboardManager`，仅在系统剪贴板不可用时回退进程内 store。旧报告中的“GUI 只使用 stub”结论已失效。

缺口：

- 大型 `main.rs` 使交互回归定位和状态性能分析困难。
- 缺桌面自动保存、崩溃恢复、多文档、拖放、窗口生命周期等可验证闭环。
- 没有自动化执行“启动 -> 绘制 -> 保存 -> 关闭 -> 重开 -> 导出”的 E2E。

- [ ] 拆分 desktop 单体文件。
- [ ] 增加自动保存/恢复、多文档和拖放。
- [ ] 建立关键桌面用户旅程 E2E。

### 4.6 Web/API、持久化与运维

已有能力：

- JWT、Argon2、OAuth state、文档/版本/分享、资产、偏好、插件状态和导出路由存在。
- 生产模式会拒绝默认或短于 32 字节的 JWT secret，以及空 CORS allowlist。
- 文档、资产和导出接口有输入/像素上限。
- 分享支持过期、撤销、owner-only revoke 和访问时间。
- OpenAPI JSON 有结构和 endpoint 集合测试。

阻塞生产的 gap：

- 用户、文档、版本、分享、资产、偏好、插件安装状态和审计日志全部在进程内；重启即丢失，多实例互不一致。
- 没有数据库 schema、migration、事务、备份/恢复和数据保留策略。
- 审计事件虽结构化并写日志，但 ring buffer 不持久。
- OpenAPI 只验证 JSON/路径/安全 scheme，未做请求响应 schema 与 handler 的契约测试。
- 没有 metrics、trace、readiness、外部依赖健康或容量告警。

- [ ] 引入持久数据库及迁移。
- [ ] 建立事务/并发控制和多租户隔离测试。
- [ ] 将审计事件送入持久、不可篡改或集中式管道。
- [ ] 增加 OpenAPI contract test。
- [ ] 增加 metrics、tracing、readiness 和运行手册。

### 4.7 安全与供应链

本轮已修复：

- [x] 限流默认不再信任可伪造的代理头。
- [x] 生产 JWT secret 有最小长度门禁。
- [x] RustSec 中 8 组可升级漏洞依赖已更新，未例外漏洞为 0。
- [x] 每周安全审计 workflow 已建立。

剩余风险：

- `quick-xml 0.39.4` 的 [RUSTSEC-2026-0194](https://rustsec.org/advisories/RUSTSEC-2026-0194.html) 和 [RUSTSEC-2026-0195](https://rustsec.org/advisories/RUSTSEC-2026-0195.html) 被临时例外。当前路径仅为 `wayland-scanner` build dependency，解析依赖自带 XML；仍应在上游允许 0.41 后立即移除例外。
- `cargo audit` 仍报告 16 个 informational/yanked 警告，主要来自 Dioxus/Wry 的 Linux GTK3 依赖、旧 rand 路径、unmaintained crate 和 Salvo 传递的 yanked `spin 0.9.8`。
- 限流仍是单进程内存实现；多实例或重启会重置状态。
- public export 虽有 8 MiB SVG、16 MP 和基础限流，仍是公开 CPU 密集入口。
- 插件默认 WASM runtime 是有限指令解释器；“Lua”实际上是受限 DSL，不应宣称通用 WASM/Lua 沙箱已经完成。
- 没有依赖许可证 allow/deny policy、source policy 或 SBOM。

- [ ] 跟踪并移除两个 quick-xml 例外。
- [ ] 升级 Dioxus/Wry/Salvo 依赖链，消化 informational/yanked 警告。
- [ ] 使用边缘网关/Redis/数据库实现分布式限流。
- [ ] 评估 export 鉴权、队列、并发隔离和请求超时。
- [ ] 增加 cargo-deny 许可证/来源策略和 CycloneDX/SPDX SBOM。

### 4.8 测试、视觉、fuzz 与 benchmark

优势：

- 当前列出 1,643 个测试，库层单元测试覆盖广。
- render 有 Inkscape fixture smoke，新视觉 target 让根目录的比较框架和 10 个 corpus fixture 真正进入 workspace test。
- DOM benchmark 可运行并已产生当前性能快照。

缺口：

- 视觉测试尚无 committed reference PNG，也不比较跨提交输出；只能发现 blank/nondeterministic，不会发现所有细微渲染变化。
- 没有 coverage 指标和最低阈值。
- fuzz target 存在，但未进入 CI，本轮也因缺 `cargo-fuzz` 未运行。
- benchmark 是手工 timing harness，没有统计分布、历史阈值或性能回归门禁。
- 没有桌面 E2E 和真实 Web API HTTP integration suite。

- [x] 视觉 corpus smoke 进入 workspace test。
- [ ] 建立分平台 reference PNG、容差和 diff artifact。
- [ ] 将短时 fuzz smoke 加入 CI，定期执行长时 fuzz。
- [ ] 引入 Criterion 或等价统计基准并保存趋势。
- [ ] 增加 coverage 和桌面/API E2E。

### 4.9 协作能力

已有 CRDT、OT、presence、awareness、comments、version history 和 transport 模型及大量单测。

但 `transport` 明确是内存 room hub；Web crate 没有 WebSocket 协作路由，Desktop 也没有完整 presence/comments/conflict UI 流程。当前应定义为“协作算法与协议原型”，不能定义为实时协作产品。

- [ ] 实现鉴权 WebSocket/同步协议和房间生命周期。
- [ ] 持久化 operation log/snapshot/version vector。
- [ ] 接入编辑器双向同步、presence、comments 和冲突 UI。
- [ ] 增加断线、乱序、重连、多客户端收敛 E2E。

### 4.10 插件与扩展

已有 manifest、permission、PluginManager、内置插件、WASM 校验/有限解释器、受限 DSL 和 UI 数据模型。

缺口：

- Desktop 只初始化并记录内置插件，未看到真实 Extensions 菜单调用 `activate_plugin` 并将 `PluginAction` 应用到主 EditorEngine 的生产路径。
- Web plugin registry 只是安装状态，不执行下载、验签、沙箱运行或升级。
- WASM 只支持有限数值指令，LuaEngine 明确不是 Lua runtime。

- [ ] 接通桌面 Extensions 入口和 action dispatch。
- [ ] 定义插件包签名、来源、版本兼容和更新策略。
- [ ] 对第三方模块做资源配额、取消、超时和权限审计。

### 4.11 导入导出互操作

SVG/PNG/JPEG/PDF/EPS/DXF 等路径存在，基础测试也较多；但 PDF importer 自身明确不是完整 PDF interpreter，图片 XObject 只保留 metadata placeholder，PDF/EPS 中仍存在跳过/占位路径。

- [ ] 用真实第三方 corpus 做 round-trip/fidelity 测试。
- [ ] 明确每种格式支持级别，不把“有函数入口”等同“完整兼容”。
- [ ] 为超大、恶意或损坏输入增加专门的资源上限与 fuzz corpus。

### 4.12 性能、可访问性、国际化与发布

性能：

- 当前 DOM benchmark 证明基本 parse/serialize 性能可测。
- 还缺大文档、滤镜、文本、图片、导出、内存峰值和交互延迟基准。

可访问性/国际化：

- 文本引擎有 BiDi、vertical text 和 CJK/RTL 相关基础能力。
- UI 源码中只有少量 `tabindex` 证据，几乎没有明确 ARIA/语义、焦点顺序和屏幕阅读器测试。
- 没有 UI 字符串资源/i18n 架构。

发布：

- Apache-2.0 license 文件已存在。
- 没有 CHANGELOG、installer、签名、公证、release workflow、版本策略或 SBOM。

- [ ] 建立容量/交互性能基线和阈值。
- [ ] 做键盘、焦点、ARIA、对比度和屏幕阅读器审核。
- [ ] 建立可本地化字符串资源和至少一个 RTL locale 验证。
- [ ] 建立跨平台安装包、签名、发布说明和回滚流程。

## 5. 按优先级汇总

### P0：当前质量基线

- [x] 全 workspace 构建、测试、fmt、all-targets Clippy 通过。
- [x] 严格 rustdoc 通过并进入 CI。
- [x] RustSec 未例外漏洞清零并进入定期 CI。
- [x] 修复限流代理头绕过和弱生产 JWT secret 校验。

### P1：阻塞生产交付

- [ ] 数据库、迁移、事务、备份和多实例一致性。
- [ ] 分布式限流、持久审计和生产可观测性。
- [ ] 明确并交付 Web 前端范围，或正式从产品范围移除。
- [ ] 桌面关键用户旅程 E2E 与崩溃/自动恢复。
- [ ] 插件入口/动作闭环及第三方供应链策略。
- [ ] 协作网络、存储和 UI 闭环。
- [ ] release packaging、签名、SBOM 和许可证策略。

### P2：质量与长期维护

- [x] 视觉 corpus smoke 接入 workspace。
- [ ] 参考 PNG 像素回归与跨平台差异策略。
- [ ] fuzz、coverage、统计 benchmark 和性能趋势。
- [ ] 拆分 `desktop/main.rs`。
- [ ] SVG filter/CSS/GPU 长尾兼容。
- [ ] 可访问性和国际化。
- [ ] 清理 16 个 RustSec informational/yanked 警告及 2 个临时例外。

## 6. 建议实施路线

### 阶段 A：先形成可持久化的单机产品

1. 定义 repository/service 边界并引入数据库迁移。
2. 持久化用户、文档、版本、分享、资产、偏好、插件状态和审计事件。
3. 增加备份恢复、并发更新和授权集成测试。
4. 完成桌面保存/重开/自动恢复 E2E。

### 阶段 B：建立可信发布门禁

1. 增加 MSRV、Windows/macOS、release build 矩阵。
2. 提交分平台视觉基线并上传 diff artifact。
3. 增加 fuzz、coverage、cargo-deny 和 SBOM。
4. 建立安装包、签名、CHANGELOG 和 release workflow。

### 阶段 C：再扩展协作、插件和 Web

1. 先决定 Web 前端是否仍是正式范围。
2. 在持久化与运维基础稳定后接入协作 WebSocket、presence 和 comments。
3. 完成插件 UI/action dispatch，再开放第三方安装与更新。
4. 最后接真实 GPU/Web 渲染器，避免同时扩大多个高风险边界。

## 7. 审计判定规则

- 只有代码已落地且对应验证通过的条目才标记 `[x]`。
- 仅有模块、类型、函数入口或单元测试，不等于产品闭环完成。
- smoke test 不等于像素回归，内存模型不等于生产持久化，命令记录器不等于真实 GPU backend。
- `cargo audit` 的两个 ignore 是带可达性理由的临时风险接受，不等于依赖已修复。
- 本报告是当前统一事实来源；其他 `_aggressive_*`、`_improve.md`、`_todos.md` 文件仅作历史计划参考。
