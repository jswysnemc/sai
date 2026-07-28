/// Codex app-server 保留的非来源客户端名称。
///
/// 使用该名称时，app-server 不会用 ACP 宿主覆盖默认的 `codex_cli_rs`
/// 来源和 User-Agent，行为与 Sai 内置内核的 Codex 客户端兼容模式一致。
const CODEX_APP_SERVER_NON_ORIGINATING_CLIENT: &str = "codex_app_server_daemon";

/// 返回外部内核在 ACP 握手中声明的客户端名称。
///
/// 参数:
/// - `engine_name`: 外部内核稳定名称
///
/// 返回:
/// - 发送给 ACP agent 的 `clientInfo.name`
pub(super) fn client_info_name(engine_name: &str) -> &'static str {
    if engine_name == "codex" {
        CODEX_APP_SERVER_NON_ORIGINATING_CLIENT
    } else {
        "sai"
    }
}
