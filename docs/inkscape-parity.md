# Sivgo 与 Inkscape 的功能差距

复核日期为 2026-11-12。以当前 Glory/Sapid 桌面端、Rust 编辑引擎和 Salvo API 为准。对照 Inkscape 1.4 系列的绘图、文件和桌面工作流。这里按用户能否完成操作来区分底层实现和实际交付。[Inkscape 1.4 发行说明](https://wiki.inkscape.org/wiki/Release_notes/1.4) · [Inkscape 操作指南](https://inkscape-manuals.readthedocs.io/en/latest/)

| 优先级 | 工作流 | Sivgo 当前状态 | 下一阶段验收 |
|---|---|---|---|
| 已交付 | 打开、保存与恢复 | 桌面支持 SVG/SVGZ 打开、导入、原子保存和另存。自动保存写入独立草稿，重启可恢复为未保存绘图。最近文件可打开、清除和剔除失效路径。 | 后续补充跨平台真实窗口和异常退出回归。 |
| P0 | SVG 保真与响应速度 | 渲染接入 CSS、渐变、文本、滤镜、遮罩和标记。画布仍从整份文档生成 CPU 位图，长文档缩放和大对象树需要实测。 | 建立 Inkscape 样稿的往返和像素基线。记录大文档的首屏、编辑和缩放延迟。把高频画布刷新迁到按视口或脏区渲染。 |
| P1 | 路径与精确编辑 | 已有节点工具、布尔、偏移、对齐、吸附和 LPE 面板。缺少 Inkscape 风格的交互式 Shape Builder、完整 XML 属性编辑器及其诊断。 | 通过选区、拖拽、键盘和撤销完成典型节点与路径任务。按真实样稿补足 LPE 参数与兼容性。 |
| P1 | 文本、颜色和滤镜 | 已有画布文字编辑、文字属性、填充描边、色板与滤镜原语链。字体预览和收藏、可搜索的调色板、滤镜效果图库仍缺。 | 用字体预览和搜索挑选字体，验证混排、字距和文本沿路径往返。为常用滤镜提供预览和可复用预设。 |
| P1 | 页面与格式互操作 | 有页面工具、页面 SVG/PDF 导出及多格式 IO。桌面导出入口分散，PDF 导入器覆盖部分操作，批量导出仍主要通过库。 | 同一导出面板能选择页面、选区、DPI 和批处理目标。以 Inkscape/PDF 实际样稿验证文本、透明度和多页数据。 |
| P1 | 专业桌面体验 | 绘图工具、文件、撤销、缩放和关闭已有语义化矢量图标；本轮补齐图层/对象的可见性、锁定、展开，以及 AI 停止、应用、丢弃和新对话图标。面板停靠、多窗口、无障碍和国际化仍需补齐。 | 真实 Windows、macOS 和 Linux 窗口中验证键盘焦点、读屏、文本输入、缩放和窄窗口布局。发布可安装的构建。 |
| P2 | 扩展与资源 | 已有资源浏览、插件 manifest 和权限模型。WASM 执行覆盖有限的数字指令，脚本是受限 DSL。 | 提供可安装、可升级、可审查权限的扩展目录，并把受支持的插件命令接入编辑历史。 |
| 产品扩展 | AI 对话编辑 | 已有模型配置、多轮对话、预览和撤销；本轮新增按 XML ID 的原子属性操作、修改前后值审查和实际 HTTP 取消。对象属性编辑保留节点 ID、选区和未涉及节点；结构修改仍用整图提案。Web 鉴权 API 返回已校验的 SVG 及可选 operations。 | 下一步支持结构化增删对象、逐项批准、流式回复、模型发现、凭证保险库及较大文档的上下文选择。当前限 256 KiB 静态 SVG，详见 [AI 协议](ai-agent.md)。 |
| 产品扩展 | 浏览器编辑与协作 | 该工作树提供 Web REST API 和内存存储。协作已有 CRDT/OT 与内存房间传输。 | 实现浏览器入口、数据库持久化、带鉴权的 WebSocket 房间和断线重连后的编辑收敛。 |

## 历史文档如何阅读

- [_report.md](../_report.md) 是 2026-07-15 的全局审计。测试、工程和生产就绪度结论是当时的快照。以现行测试和实际入口判定当前状态。
- [_improve.md](../_improve.md) 记录 2026-09-02 的缺陷修复。SVG 往返、CSS 样式表、变换顺序和崩溃用例适合作为回归基线。当前渲染树只保留 tiny-skia 后端。较早 [_aggressive_render_backends_tasks.md](../_aggressive_render_backends_tasks.md) 的已勾选项描述的是曾经的命令回放。
- [_dixous_report.md](../_dixous_report.md) 审计的是 Dioxus 时代的布局和渲染。旧代码在 `old-dioxus`。当前桌面从提交 `487b596` 迁到 Glory/Sapid，后续 `6da0775` 修复文字输入与保存往返。大文档的派生状态和画布性能仍适合在当前宿主上重新测量。
- [_aggressive_import_export_tasks.md](../_aggressive_import_export_tasks.md)、[_aggressive_plugin_system_tasks.md](../_aggressive_plugin_system_tasks.md) 和 [_aggressive_collaboration_tasks.md](../_aggressive_collaboration_tasks.md) 记录库与模型的验收。桌面入口、运行时、网络和持久化能力以现行实现为准。
- 图层、吸附、描边、显示模式、LPE、滤镜、资源与文字各有独立的 `_aggressive_*_tasks.md` 记录，适合用来追溯参数与测试。[_aggressive_pdf_import_tasks.md](../_aggressive_pdf_import_tasks.md) 的目标是简单 PDF。[_aggressive_web_production_tasks.md](../_aggressive_web_production_tasks.md) 记录 API 加固，数据库与浏览器前端仍是独立交付项。
- [_frontend_e2e_tasks.md](../_frontend_e2e_tasks.md) 列出语义树、画布和交互层测试。跨平台真实窗口回归与稳定参考图像仍需要单独建设。

文件恢复已接通。下一阶段优先量化大文档画布性能，并用同一批跨编辑器样稿推进页面导出、文字和路径保真。AI 建议端点与后续协作可分别推进，当前的预览和撤销流程可作为后续对象级 AI 操作的验收基线。
