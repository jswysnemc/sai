use super::ToolRegistry;
use serde_json::{json, Value};

impl ToolRegistry {
    /// 【渠道消息】【来源注入】参数 name/args 为工具及已授权参数，返回无；来源只能取自宿主身份。
    pub(super) fn inject_channel_origin(&self, name: &str, args: &mut Value) {
        if name != "send_channel_message" {
            return;
        }
        let identity = self
            .plugin_session_id
            .as_deref()
            .unwrap_or(&self.session_id);
        let (session_id, subagent_id) = match identity.split_once("/subagent/") {
            Some((parent, child)) => (parent, Some(child)),
            None => (identity, None),
        };
        if !session_id.is_empty() {
            if let Some(object) = args.as_object_mut() {
                object.insert("_sai_channel_origin".into(), json!({
                    "session_id":session_id, "subagent_id":subagent_id, "state_dir":self.session_key,
                }));
            }
        }
    }
}
