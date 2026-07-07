# Sivgo 项目完成度与缺口统一审计报告

审计日期: 2026-07-07
审计对象: 当前工作区 `D:\Works\taidge\sivgo`
基线状态: `main` 分支, 本轮报告随最终提交形成可审查基线

## 0. 本轮改进进度

- [x] 修复 `cargo fmt --all -- --check` 阻塞项, 并执行 `cargo fmt --all`。
- [x] 修复 `cargo clippy --workspace -- -D warnings` 阻塞项。
- [x] 额外修复 `cargo clippy --workspace --all-targets -- -D warnings` 暴露的 test/bench target lint。
- [x] 补齐 `LICENSE-MIT` 与 `LICENSE-APACHE`。
- [x] 修正 README/architecture 中 Web/WASM/PostgreSQL/Rust 版本等与当前代码不一致的描述。
- [x] GitHub Actions 增加 `cargo fmt --all -- --check`。
- [x] 修正 `just fuzz` 默认 target 为现有 `svg_parse`。
- [x] 删除指向缺失 `render_bench.rs` 的旧 `benches/Cargo.toml`; 当前 workspace bench 位于 `crates/dom/benches/parse_serialize.rs`。
- [x] 为 Web public routes 增加进程内基础限流。
- [x] 为 document shares 增加 `expires_at`, `revoked_at`, `created_by`, `last_accessed_at`, active-token 解析和 owner-only revoke API。
- [x] 清理当前 staged/unstaged/untracked 工作区并形成可审查提交边界。
- [ ] Web 持久化、生产级分布式限流、真实 Web/WASM 前端、协作 WebSocket/UI、插件动作落地、视觉回归 CI 等生产化缺口仍未完成。

## 1. 结论摘要

Sivgo 当前更像一个覆盖面很广的 SVG 编辑器技术原型, 而不是可发布的生产产品。底层 Rust workspace 结构完整, 14 个 crate 都能编译, 单元/集成/doctest 覆盖面较大。本轮改进后, `cargo check --workspace --all-targets`, `cargo build --workspace`, `cargo test --workspace`, `cargo fmt --all -- --check`, `cargo clippy --workspace -- -D warnings`, `cargo clippy --workspace --all-targets -- -D warnings` 均通过。核心能力包括 DOM、样式、渲染、路径操作、文本、滤镜、导入导出、编辑引擎、桌面 UI、Web REST API、插件和协作模型。

但当前项目不能判定为交付完成:

- 当前大范围工作区变更已收敛为本轮提交边界, 包括既有 UI/Web/桌面平台模块删除和本轮 Web 分享/限流改进。
- 原 CI 阻塞项已修复, 本轮提交前已重新跑通全 workspace 质量门禁。
- README 与 architecture 的当前实现描述已修正; 但产品长期目标和已删除 Web 前端/PWA/浏览器文件模块仍需项目层面确认。
- Web、协作、插件、视觉回归、发布合规仍有明显生产化 gap。

粗略完成度判断:

| 范围 | 完成度估计 | 说明 |
|---|---:|---|
| Rust 库层/编辑引擎原型 | 65-75% | 模块广、测试多, 但还有 SVG fidelity 与集成缺口 |
| 桌面端可用原型 | 45-55% | GUI/CLI 存在, 但 `main.rs` 单体化严重, 平台能力模块大量删除 |
| Web 后端 API 原型 | 35-45% | REST API 有基础安全改进, 但存储/限流/生产部署不足 |
| Web 前端/WASM 产品 | 10-20% | 当前代码树基本没有实际前端入口 |
| 生产发布/CI/合规 | 40-50% | fmt/clippy/license 基线已修复, 但 benchmark/fuzz/visual/security/release 门禁仍不足 |

总体: 技术原型约 60%+, 生产级 SVG 编辑器约 35-45%。

## 2. 本次验证结果

已执行:

| 命令 | 结果 | 备注 |
|---|---|---|
| `cargo metadata --no-deps --format-version 1` | 通过 | 当前 workspace 有 14 个成员 |
| `cargo check --workspace --all-targets` | 通过 | 所有已声明 target 可编译 |
| `cargo build --workspace` | 通过 | dev build 成功 |
| `cargo test --workspace` | 通过 | 单元/集成/doctest 均通过 |
| `cargo fmt --all -- --check` | 通过 | 本轮已格式化全部 Rust 文件 |
| `cargo clippy --workspace -- -D warnings` | 通过 | CI 形态 clippy 已全绿 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过 | all-targets clippy 已全绿 |

代码规模快照:

| crate | Rust 文件数 | 代码行数 |
|---|---:|---:|
| core | 97 | 30216 |
| render | 15 | 10935 |
| dom | 9 | 9624 |
| desktop | 4 | 7673 |
| ui | 21 | 5883 |
| filters | 22 | 5418 |
| text | 12 | 5362 |
| io | 11 | 6236 |
| math | 17 | 5604 |
| web | 24 | 3841 |
| plugin | 11 | 2777 |
| path-ops | 12 | 2489 |
| trace | 9 | 1910 |
| style | 7 | 3044 |
| total | 271 | 101012 |

## 3. 可审计维度清单

当前项目建议至少从这些维度审计:

1. 产品范围与完成度: README 声称的功能和实际代码可达功能是否一致。
2. 构建、CI 与发布就绪度: check/build/test/fmt/clippy/release/package 是否可重复。
3. Git 工作区与交付基线: staged/unstaged/untracked 是否清晰, 是否可复现。
4. 架构与模块边界: crate 依赖是否合理, 关键代码是否单体化。
5. SVG 兼容性与渲染 fidelity: DOM/样式/滤镜/文字/图片/marker/pattern/mask 是否按规范。
6. 编辑器核心能力: 工具、命令、撤销重做、选择、snapping、LPE、图层/页面。
7. 桌面端 UX 与平台能力: 文件对话框、剪贴板、多文档、拖放、最近文件、快捷键、设置。
8. Web/API 生产化: 鉴权、授权、CORS、限流、持久化、OpenAPI、错误模型。
9. 数据持久化与迁移: 文档、资源、偏好、分享、版本历史是否可持久、安全迁移。
10. 协作能力: CRDT/OT/presence/comments 是否接入真实网络、UI 和存储。
11. 插件/扩展能力: 安装、权限、WASM/Lua 执行、UI 集成、插件动作落地。
12. 导入导出互操作: SVG/PNG/JPEG/PDF/EPS/DXF/WebP/PDF import 的真实覆盖。
13. 测试质量: 单元、集成、视觉回归、E2E、fuzz、benchmark、snapshot 是否进入 CI。
14. 性能与内存: 大 SVG、滤镜、文本、导出、Web 请求体、缓存/增量渲染。
15. 安全: Web auth、上传、导出 DoS、路径/文件、插件沙箱、供应链。
16. 可维护性: clippy、格式、重复数据模型、死代码、文档同步。
17. 可访问性与国际化: keyboard nav、ARIA、i18n、RTL/CJK 文本。
18. 合规与分发: license 文件、第三方依赖、安装包、版本策略。

## 4. 分维度审计

### 4.1 产品范围与文档一致性

现状:

- `Cargo.toml` 声明 14 个 workspace members, 与 README 中的 14 crate 基本一致。
- `Cargo.toml:24` 声明 `rust-version = "1.89"`, README 已同步为 Rust 1.89+。
- README 与 architecture 已改为描述当前状态: 桌面 Dioxus app + Salvo REST API backend; 当前 worktree 不包含 Web/WASM client。
- README 已移除 PostgreSQL 前置要求, 并说明当前 Web backend 使用内存态 store。

判断: 当前文档已比原报告对齐代码树; 但产品目标态是否恢复 Web/WASM frontend 仍需确认。

### 4.2 构建、CI 与质量门禁

现状:

- `cargo check`, `cargo build`, `cargo test`, `cargo fmt --all -- --check`, `cargo clippy --workspace -- -D warnings`, `cargo clippy --workspace --all-targets -- -D warnings` 均通过。
- `.github/workflows/ci.yml` 已安装 `rustfmt` component 并新增 format check。
- `just lint` 仍覆盖 clippy + fmt-check。

判断: 本轮已修复原 P0 质量门禁阻塞并形成提交边界; 下一步风险主要是未接入的视觉/fuzz/benchmark/security/release 门禁。

### 4.3 Git 工作区风险

现状:

- 当前分支是 `main`; 本轮把此前 staged/unstaged/untracked 变更纳入同一个可审查提交边界。
- 提交内容包含既有 UI/Web/桌面平台模块删除、质量门禁修复、文档/许可证/CI 修正, 以及本轮 Web 分享与公开路由限流改进。
- `_report.md` 是当前统一审计依据; `_improve.md` 仍保留为历史改进记录, 不作为完成度事实来源。

判断: Git 基线风险已从“工作区混乱”降为“单个大提交需要重点 review”。后续更适合按 Web 持久化、桌面 UX、插件、协作、视觉回归等主题拆分增量提交。

### 4.4 架构与维护性

优势:

- Workspace 分层清晰: math/dom/style/render/path/text/filters/trace/io/core/plugin/ui/desktop/web。
- 大部分能力在独立 crate 中, 库层测试覆盖较多。
- `core` 已公开 collab、LPE、commands、tools 等模块, `io` 已重导出 PDF import 和 bitmap trace。

主要问题:

- `crates/desktop/src/main.rs` 当前约 7693 行, `fn App()` 从 `main.rs:5564` 开始, 包含大量 `use_signal` 与内联 `rsx!`。这会提高 UI 维护、性能分析和回归定位成本。
- UI 模块数量从历史任务文档里的大量 panel/子模块, 缩减为 `crates/ui/src/lib.rs:26-45` 的 20 个左右公开模块, 同时很多已暂存删除。需要确认这是有意瘦身还是误删。
- `crates/desktop/src/plugins.rs:25-43` 只提供列表/激活 helper; `main.rs:2948-2955` 只初始化并 log 插件, 未看到真实 Extensions 菜单或插件动作应用路径。

判断: 架构方向合理, 但桌面端和插件接入仍偏原型, 单体 UI 是后续维护主要风险。

### 4.5 Web/API 与生产化

已具备:

- JWT 使用 `jsonwebtoken` HS256, token 有 `exp`。
- 密码使用 Argon2。
- OAuth state 使用签名短期 JWT。
- 文档、资产、偏好、插件安装、分享等 REST route 基本存在。
- 上传/文档/export 有请求体或像素上限。

缺口:

- 全部核心存储仍是内存态: `Store` 使用 `Arc<RwLock<HashMap<...>>>`, `AssetStore`, `PreferencesStore`, `PluginRegistry` 也都是内存态。进程重启即丢数据。
- `router.rs` 已为 public API tree 挂载进程内 `RateLimiter`, 覆盖 `register`, `login`, `oauth`, `export`, `shared`, `plugins` 等公开路由; 生产环境仍建议用 edge/cache/数据库实现分布式限流。
- `handlers/export.rs:36` 明确 export 是 unauthenticated, 目前有 8 MiB SVG 和 16MP 限制, 但仍可能被公开接口消耗 CPU。
- `config.rs:70-73` 生产环境 CORS 未配置时只是 warn 后 fallback permissive, 不拒绝启动。
- 分享模型已增加 `expires_at`, `revoked_at`, `created_by`, `last_accessed_at`; store 解析 token 时会过滤过期/撤销分享并记录访问时间, handler 已提供 owner-only revoke API。
- OpenAPI 与实际响应仍需复核: 历史输出里 OAuth token schema 和当前 `AuthResponse` 形态可能存在残留不一致风险。

判断: Web 后端是开发/演示级别, 不是生产服务。必须引入数据库、迁移、限流、持久会话/刷新策略、生产 CORS 策略和审计日志。

### 4.6 Web 前端/WASM

现状:

- 原 README 曾声称有 Dioxus WASM Web app; 本轮已降级为当前实际状态描述。
- 当前 workspace 中 `sivgo-web` 是 Salvo server binary, 未看到 Dioxus Web frontend target。
- Web UI/PWA/浏览器文件/触屏/响应式相关模块在当前 worktree 被删除。
- `render` 中 Canvas2D/WebGPU backend 是命令流 + TinySkia CPU replay, 不是浏览器真实渲染管线。

判断: Web 前端完成度很低。需要明确产品路线: 要么继续只保留 REST API server, 要么恢复/重建 Web client。

### 4.7 渲染与 SVG 兼容

优势:

- TinySkia 后端可渲染基本形状、路径、文字、图片、patterns、markers、clips、masks、filters 等, 并有 render smoke tests。
- 滤镜和 LPE 覆盖面很广, `filters`、`core::lpe` 的单元测试很多。

缺口:

- `renderer.rs:3559-3569` 的 `feComponentTransfer` 因 DOM 未解析 `feFunc*` 子元素, 当前是 identity passthrough。
- `renderer.rs:3578` 和 `3591` lighting filter 的 `light_color` 硬编码为白色。
- `renderer.rs:3627-3631` 仍会跳过未知/未实现 filter primitive。
- PDF import 在 `io/lib.rs:13` 自述为 stub/minimal, `pdf_import.rs:6-78` 也说明 unsupported PDF feature 会以 image placeholder 保留。
- Vello/WebGPU/Canvas2D backend 暴露的是 command stream + CPU replay, 不是实际 GPU/browser adapter。

判断: 渲染可做 demo 和大量基础导出, 但距离 Inkscape 级 SVG 兼容还有长尾工作。

### 4.8 桌面端 UX 与平台能力

已具备:

- 默认 GUI 使用 Dioxus Desktop, CLI 支持 `render` 和 `info` (`main.rs:2841-2940`)。
- 系统剪贴板包装存在, 错误可返回。
- UI 面板包括 layers/properties/resources/filter/lpe/pages/preferences 等。

缺口:

- `main.rs` 单文件过大, UI state 和业务命令混杂。
- 已暂存删除的平台能力模块包括 autosave、batch、drag_drop、file_dialogs、multi_doc、recent_files_os、window。
- `clipboard.rs:7-11` 和 `103-123` 提供 thread-local stub API, 而 `main.rs` 搜索结果显示 GUI 动作仍大量使用 `copy_*_to_stub_clipboard` / `paste_*_from_stub_clipboard`。这意味着真实系统剪贴板可能没有完整接入到 GUI 菜单动作。
- 多文档、自动保存、拖放、OS 最近文件、窗口管理等被删除后需要重新评估真实可用性。

判断: 桌面端有可运行 GUI 和 CLI, 但离稳定桌面产品仍有平台集成 gap。

### 4.9 协作能力

已具备:

- `core::collab` 已有 CRDT、OT、presence、awareness、comments、version history、transport。
- 单元测试覆盖较多。

缺口:

- `collab/transport.rs:1-4` 说明是 in-memory room hub, 类似确定性 WebSocket broker, 主要用于测试/local embedded。
- 当前 `crates/web` 没有真实 WebSocket route 或协作 API。
- 桌面/Web UI 没看到 presence cursor、comments、conflict resolution 的实际用户流程。
- 协作文档与普通 `EditorEngine`/`SvgDocument` 的双向同步边界仍不清晰。

判断: 协作是库层模型完成, 产品级实时协作未完成。

### 4.10 插件/扩展

已具备:

- `sivgo-plugin` 有 manifest、permission、PluginManager、PluginRegistry、builtin plugins、plugin UI primitives。
- WASM sandbox 会验证模块、限制 import/memory, 并有 bounded interpreter。

缺口:

- `wasm_sandbox.rs:1-7` 明确默认 runtime 是受限解释器; `250-383` 只解析并执行有限导出的数值函数, 不是完整 WASM runtime。
- `lua_engine` 名称与真实 Lua runtime 容易混淆, 当前更像轻量 DSL/表达式边界。
- 桌面端只初始化并 log built-in plugins, 未看到菜单入口和 `PluginAction` 到 `EditorEngine` 的完整应用路径。
- Web plugin registry 只是用户安装状态模型, 不是真正下载、沙箱运行和更新机制。

判断: 插件系统是 API/模型级原型, 不是可供第三方稳定使用的扩展平台。

### 4.11 测试、视觉回归、fuzz、benchmark

优势:

- `cargo test --workspace` 全绿。
- render crate 有 Inkscape fixture smoke test。
- core/filter/text/trace 等单元测试数量可观。

缺口:

- `tests/visual/mod.rs:1-5` 是视觉回归框架, 但根目录只是 workspace (`Cargo.toml:1-18`), 没有 root package/test target; `cargo test --workspace` 不会自动执行这套 root `tests/visual`。
- `tests/visual/mod.rs:1-5` 尚未变成 CI 中真实执行的视觉回归 target, 仍没有 reference PNG 生成/比较门禁。
- `justfile` 默认 fuzz target 已修正为 `svg_parse`, 但本轮未实际运行 fuzz。
- 旧的 `benches/Cargo.toml` 已删除; 当前 workspace bench 位于 `crates/dom/benches/parse_serialize.rs`, 但 benchmark 尚未作为 CI 门禁。
- CI 已包含 format, 但仍没有 visual、fuzz、benchmark、coverage、security audit。

判断: 单元测试强, 但产品质量门禁弱。

### 4.12 安全

改进点:

- JWT/Argon2/OAuth state 已比早期报告里的自制 crypto 明显更好。
- 资源上传、文档体、export 像素面积有初步限制。

主要风险:

- 公开 login/register/oauth/export/shared/plugins 已有基础进程内限流; 生产级分布式限流仍需补。
- 生产 CORS 未配置时不拒绝启动。
- Web 全内存存储, 缺会话撤销、审计日志、备份、隔离策略。
- 插件 sandbox 不完整, 第三方代码执行能力必须继续收紧。
- public share token 无过期/撤销。

判断: 安全基础比原型早期更好, 但生产暴露仍不建议。

### 4.13 合规与发布

缺口:

- `LICENSE-APACHE` 和 `LICENSE-MIT` 已补齐。
- 没有安装包/发布流水线/版本变更记录。
- 没有依赖审计或 SBOM。

判断: 不能直接对外分发。

## 5. 优先级缺陷与 gaps

### P0 - 阻塞交付

- [x] 当前 worktree 不干净且包含大量 staged 删除。已在本轮收敛为可审查提交边界。
- [x] 修复 `cargo fmt --all -- --check` 失败。
- [x] 修复 `cargo clippy --workspace -- -D warnings` 失败。
- [x] 补齐缺失许可证文件, 修复 README license 链接无目标。

### P1 - 阻塞生产化

- [ ] Web 数据、资产、偏好、插件安装状态全为内存存储。
- [x] 修正 README 中 Web client/WASM/PostgreSQL 与当前代码不一致的描述。
- [x] 为公开 API 增加基础进程内 rate limiting; 生产级分布式限流仍需补。
- [x] 分享链接增加过期、撤销和访问审计字段。
- [ ] 桌面平台能力模块大量删除, 需要确认真实 UX 是否回退。
- [ ] 插件只初始化/log, 缺 UI 入口和动作落地。
- [ ] 协作只在 core 模型层, 无真实 WebSocket/API/UI。
- [x] 修复 fuzz 默认 target 与 bench stale manifest 不可靠。
- [ ] 视觉回归/fuzz/benchmark 未接入 CI 门禁。

### P2 - 质量与长期维护

- [ ] `desktop/main.rs` 单体化, UI 状态和业务动作高度耦合。
- [ ] SVG filter/render 长尾兼容仍有 passthrough/skip。
- [ ] GPU/Web backend 是 CPU replay command stream, 非真实后端。
- [x] 修正文档与任务报告 stale/不一致。
- [x] 修正 Rust 版本文档过期。

## 6. 建议路线

### 第一阶段: 恢复可交付基线

- [x] 清理 git 状态: 把 staged 删除和 unstaged 修改收敛为明确 commit 边界。
- [x] 运行 `cargo fmt --all` 或手工修复格式。
- [x] 修复 clippy 错误, 并重复跑 `cargo clippy --workspace -- -D warnings`。
- [x] 补 `LICENSE-MIT` 和 `LICENSE-APACHE`。
- [x] 修正 README: Rust 版本、Web client 状态、PostgreSQL 状态、visual/bench/fuzz 状态。
- [x] 将 `cargo fmt --all -- --check` 加入 GitHub Actions。
- [x] 修正 `just fuzz` target 名, 删除 stale bench manifest。

### 第二阶段: 产品闭环

- [ ] 决定 Web 方向: 只保留 REST server, 或恢复 Dioxus Web/WASM frontend。
- [ ] 引入数据库层与迁移, 替换 Web in-memory store。
- [x] 给 login/register/oauth/export/shared/plugins 增加基础进程内限流。
- [ ] 增加生产级分布式限流、审计日志和生产配置拒绝策略。
- [x] 把分享模型扩展为可撤销、可过期、可审计。
- [ ] 接通插件 UI 入口和 `PluginAction` 应用路径。
- [ ] 接通协作 WebSocket/API/UI, 或把协作文档降级为内部实验模块。

### 第三阶段: 质量门禁

- [x] 把 `cargo fmt --all -- --check` 加入 GitHub Actions。
- [x] 修正 `just fuzz` target 名, 让 fuzz recipe 指向现有 target。
- [x] 修复或删除坏掉的 `benches/Cargo.toml`。
- [ ] 把 benchmark 纳入 workspace 或显式 recipe 并接入 CI。
- [ ] 将 `tests/visual` 做成真实 test target, 生成并校验 reference PNG。
- [ ] 增加 smoke E2E: 桌面启动、绘制、保存、导出、重新打开。
- [ ] 增加 security audit: `cargo audit`/依赖许可证扫描/SBOM。

## 7. 最终判断

项目已经有相当多的底层工程量, 不是空壳。它可以作为 SVG 编辑器内核和桌面原型继续推进。但当前状态距离“完整产品”还有明显差距, 最大风险不是某一个算法缺失, 而是 Web/协作/插件/视觉回归这些产品闭环尚未完成。

短期最有价值的动作是按主题继续拆分后续改进: 先补 Web 持久化与生产安全边界, 再处理桌面平台能力回归、插件动作落地、协作网络闭环和视觉回归 CI。
