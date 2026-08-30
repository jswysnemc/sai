use serde::{Deserialize, Serialize};

/// 【会话网格】【配置结构】跨会话消息收发开关。
///
/// 网格工具（mesh_send / mesh_recv / mesh_reply）能触碰别的会话与别的
/// 子智能体的状态。默认只允许投递给当前会话自己，跨越会话边界必须显式开启
/// `mesh.cross_session`，否则任何一个 agent 都能往任意会话注入消息。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeshConfig {
    /// 是否允许网格消息跨越会话边界；默认关闭。
    #[serde(default)]
    pub cross_session: bool,
}
