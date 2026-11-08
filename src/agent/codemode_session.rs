use super::Agent;

impl Agent {
    /// 【CodeMode】【热切换】同步最新会话模式，使下一次模型请求和工具调用使用正确入口
    /// 参数: self 为当前会话；返回: 无
    pub(super) fn sync_codemode_session(&mut self) {
        let mode = self.mode();
        self.tool_visibility.set_session_mode(mode);
        crate::tools::codemode::register_for_agent(&mut self.tools, &self.config, mode);
    }
}
