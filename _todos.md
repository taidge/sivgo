# 数据结构激进审核任务

更新日期: 2026-06-01

## 审核范围

本轮只处理会跨模块、序列化、API 暴露或长期持久化的数据结构；工具内部拖拽状态、临时命令 undo 状态不作为结构兼容层处理。

## 任务清单

- [x] 删除 `crates/web/src/schema.rs` 这套未接入且字段命名冲突的重复 Web 数据模型，保留 `models.rs` 作为唯一 API 模型入口。
- [x] 移除 `AssetStore` 字符串 ID 兼容 API，所有资产读写改为 `Uuid` 强类型，并让下载返回专用数据结构。
- [x] 修正资产上传/下载处理器字段语义：上传响应返回 `AssetInfo`，下载路径参数严格解析为 `Uuid`，base64 字段实际解码为二进制。
- [x] 删除 OAuth legacy 配置/Provider/TokenRequest 兼容结构，路由和 handler 只使用 `OAuthClient + OAuthProvider + OAuthConfig`。
- [x] 将 Web preferences 和 plugin registry 的 user id 存储键从 `String` 改为 `Uuid`，删除调用层反复 `to_string()` 的兼容形态。
- [x] 拆分插件目录数据结构里的上下文状态：从 `PluginInfo` 删除 `installed` 字段，已安装列表由端点语义表达。
- [x] 重命名分享对话框中的 `SharePermission` 结构，避免和 API `SharePermission` 枚举冲突；`document_id` 改为 `Uuid`。
- [x] 删除 snapping 的旧 grid-only `snap()` API 和 backward-compat 访问器，测试改用 `snap_all`。
- [x] 更新 OpenAPI 中受影响的字段定义，使资产、插件、OAuth 行为与真实模型一致。
- [x] 运行相关 crate 测试/检查，修复由激进删除带来的编译错误。
