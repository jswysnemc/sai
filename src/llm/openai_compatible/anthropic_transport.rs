use super::*;

impl OpenAiCompatibleClient {
    /// 发送一次 Anthropic Messages 请求。
    ///
    /// @param url Messages API 地址
    /// @param request 已应用思考与自定义字段的请求体
    /// @param claude 是否使用 Claude Code 模拟协议
    /// @param session_id Claude Code 会话 UUID
    /// @param api_key 本次请求固定使用的密钥
    /// @returns 服务端 HTTP 响应
    pub(super) async fn send_anthropic_request(
        &self,
        url: &str,
        request: &Value,
        claude: bool,
        session_id: &str,
        api_key: &str,
    ) -> Result<reqwest::Response> {
        let builder = if claude {
            // 【Claude】【Messages 请求头】1. 使用 Claude Code 通道头
            let user_agent = resolve_provider_user_agent(&self.provider);
            let request = self
                .client
                .post(url)
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .header(
                    "anthropic-beta",
                    claude_code_beta_header(self.provider.claude_1m_context),
                )
                .header("anthropic-dangerous-direct-browser-access", "true")
                .header("User-Agent", user_agent)
                .header("x-app", "cli")
                .header("x-claude-code-session-id", session_id)
                .header("x-stainless-lang", "js")
                .header("x-stainless-package-version", "0.81.0")
                .header("x-stainless-runtime", "node")
                .header("x-stainless-runtime-version", "v24.3.0")
                .header("x-stainless-retry-count", "0")
                .json(request);
            // 【Claude】【Messages 请求头】2. 合并供应商自定义请求头
            with_provider_extra_headers(request, &self.provider)
        } else {
            let request = apply_provider_user_agent(
                self.client
                    .post(url)
                    .header("x-api-key", api_key)
                    .header("anthropic-version", "2023-06-01")
                    .json(request),
                &self.provider,
            );
            with_provider_extra_headers(request, &self.provider)
        };
        Ok(builder.send().await?)
    }
}
