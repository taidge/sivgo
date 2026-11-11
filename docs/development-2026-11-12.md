# 2026-11-12 开发复核与交付

本轮以当前工作树的 Glory/Sapid 桌面端、SVG 引擎、AI 客户端和 Salvo API
为范围，复核 README、架构、Inkscape 对照、历次审计与专项任务，并检查相关实现。
历史文档的勾选表示当时的库或工作流验收，不能直接视为当前产品已完成。

## 依赖核验

直接查询 crates.io registry API：最新稳定发行版为
[glory 0.7.0](https://crates.io/crates/glory/0.7.0) 与
[sapid 0.7.1](https://crates.io/crates/sapid/0.7.1)。当前 manifest 和 Cargo.lock
已经使用这两个版本，`cargo update -p glory -p sapid` 没有新的框架版本可升级。
保留现有锁定依赖，避免带入无关的传递依赖调整。

按 workspace 的 `cargo metadata --no-deps --format-version 1`，本次实际产物目录为
项目内 `target`。构建继承调用者环境，未调整 Cargo home、target 或临时目录。

## 本轮已交付

- AI 对话原有模型配置、多轮上下文、预览和撤销继续保留。新增按 XML ID 的属性
  设置/移除事务，整体校验后才形成提案，拒绝重复目标、缺失对象和危险内容。
- 桌面对象编辑保留原内部 NodeId、层级、选区及未涉及节点；临时选区 XML ID
  只用于请求定位。新增属性旧值/新值审查；结构修改明确标示整图替换。
- 停止按钮取消实际 HTTP future 并恢复输入。切换绘图或重置会话也取消请求；
  取消后的后台通知不能生成过期提案。原有源图一致性检查继续保护应用阶段。
- Salvo 建议 API 复用对象操作解析，返回已生成的 SVG 和可选 operations，OpenAPI
  已同步。接口仍不修改存储的文档，调用者选择一种表示应用。
- 图层/对象可见性、锁定、展开、对象类型及拖拽手柄使用图标；AI 新对话、停止、
  应用和丢弃使用统一图标。关闭继续使用 Sapid close button，操作保留可访问名称。
- 更新 [AI 接入协议](ai-agent.md)、[架构](architecture.md)、
  [Inkscape 差距](inkscape-parity.md) 和前端验收记录。

## 仍需推进的产品缺口

| 顺序 | 缺口与证据 | 下一阶段验收 |
|---|---|---|
| P0 | 画布仍生成整图 CPU 位图，视觉语料尚无提交的跨编辑器参考图。历史测试通过不能说明与 Inkscape 像素一致。 | 提交 SVG 往返样稿和参考图，量化大文档首屏/缩放/编辑延迟，再推进视口或脏区渲染。 |
| P1 | 路径库覆盖多，但完整 XML 属性编辑器、Shape Builder 工作流不足。 | 真实窗口中完成节点/路径任务，校验键盘、撤销和保存后重开。 |
| P1 | 导出入口分散，PDF 导入仍覆盖有限子集；文字、LPE 和滤镜需要真实样稿。 | 统一页面/选区/DPI/批量导出，建立文本、透明度和多页互操作样稿。 |
| P1 | 原生界面已有语义测试，但跨平台系统窗口、无障碍、输入法、停靠和安装包仍未完整验收。 | Windows/macOS/Linux 实窗回归、安装升级和窄窗口布局验收。 |
| P2 | Web 只有 REST API，数据内存化；协作模型不等于可用网络产品。插件 runtime 覆盖有限。 | 浏览器客户端、持久化和鉴权房间；可安装插件及权限审查和撤销闭环。 |
| AI | 当前对象工具仅编辑属性，结构变化仍为整图替换，SVG 限 256 KiB。 | 结构化增删、逐项批准、流式回复、模型发现、凭证保险库和大文档上下文选择。 |

## 验证

最终全工作区测试通过 1,856 项，失败 0 项；严格 Clippy 零警告，格式及 diff
检查通过。命令为 `cargo test --workspace --locked --offline`、
`cargo clippy --workspace --all-targets --locked --offline -- -D warnings`、
`cargo fmt --all -- --check` 与 `git diff --check`。AI 使用本地模拟
OpenAI-compatible provider，覆盖真实 HTTP 请求、鉴权头、上下文、连续对话、
对象变更、候选预览、应用、选区/标识保留、撤销/重做及连接取消。
使用 Apace 光栅渲染检查审查界面；这不等同于系统窗口驱动测试。
未使用真实付费 API Key，因此没有宣称完成某一云厂商/模型的线上兼容验收。
