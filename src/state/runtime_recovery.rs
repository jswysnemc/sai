use super::StateStore;
use crate::runtime_recovery::{
    self, NewRuntimeProcessEventInput, NewRuntimeProcessRecord, NewRuntimeRecoveryRecord,
    RemoteControlState, RemoteControlStateUpsert,
};
use anyhow::Result;
use serde_json::Value;

impl StateStore {
    /// 写入当前会话运行时进程记录。
    ///
    /// 参数:
    /// - `process`: 待写入运行时进程记录
    ///
    /// 返回:
    /// - 写入是否成功
    pub(crate) fn record_runtime_process(&self, process: NewRuntimeProcessRecord) -> Result<()> {
        runtime_recovery::record_process(&self.conv_db, process)?;
        Ok(())
    }

    /// 追加当前会话运行时进程事件。
    ///
    /// 参数:
    /// - `event`: 待写入运行时进程事件
    ///
    /// 返回:
    /// - 写入事件的序号
    pub(crate) fn append_runtime_process_event(
        &self,
        event: NewRuntimeProcessEventInput,
    ) -> Result<i64> {
        let event = runtime_recovery::append_next_process_event(&self.conv_db, event)?;
        Ok(event.seq)
    }

    /// 写入当前会话运行时恢复记录。
    ///
    /// 参数:
    /// - `record`: 待写入恢复记录
    ///
    /// 返回:
    /// - 写入是否成功
    pub(crate) fn record_runtime_recovery(&self, record: NewRuntimeRecoveryRecord) -> Result<()> {
        runtime_recovery::record_recovery(&self.conv_db, record)?;
        Ok(())
    }

    /// 审计当前会话运行时进程序列缺口。
    ///
    /// 返回:
    /// - 新写入的恢复记录数量
    pub(crate) fn audit_runtime_sequence_gaps(&self) -> Result<usize> {
        runtime_recovery::audit_sequence_gaps(&self.conv_db, &self.session_id)
    }

    /// 应用命令模式退出时的运行时资源策略。
    ///
    /// 返回:
    /// - 标记为 detached 的进程数量
    pub(crate) fn apply_command_mode_runtime_exit_policy(&self) -> Result<usize> {
        runtime_recovery::apply_command_mode_exit_policy(&self.conv_db, &self.session_id)
    }

    /// 应用网关连接关闭时的运行时资源策略。
    ///
    /// 参数:
    /// - `gateway_id`: 网关标识
    ///
    /// 返回:
    /// - 连接关闭策略执行结果
    pub(crate) fn apply_gateway_connection_close_policy(
        &self,
        gateway_id: &str,
    ) -> Result<runtime_recovery::ConnectionClosePolicyOutcome> {
        runtime_recovery::apply_connection_close_policy(
            &self.conv_db,
            &self.session_id,
            runtime_recovery::OwnerKind::Gateway,
            gateway_id,
        )
    }

    /// 记录当前会话网关 transport 断开观察事件。
    ///
    /// 参数:
    /// - `gateway_id`: 网关标识
    /// - `reason`: 断开原因
    /// - `last_sequence`: 最近一次 transport 序号
    ///
    /// 返回:
    /// - 写入是否成功
    pub(crate) fn record_gateway_transport_close(
        &self,
        gateway_id: &str,
        reason: &str,
        last_sequence: Option<u64>,
    ) -> Result<()> {
        runtime_recovery::record_gateway_transport_close(
            &self.conv_db,
            &self.session_id,
            gateway_id,
            reason,
            last_sequence,
        )
    }

    /// 推进当前会话网关 transport cursor 和 ack。
    ///
    /// 参数:
    /// - `gateway_id`: 网关标识
    /// - `cursor_seq`: 可选已接收序号
    /// - `acked_seq`: 可选已处理序号
    ///
    /// 返回:
    /// - 推进后的 transport 状态
    pub(crate) fn advance_gateway_transport_cursor(
        &self,
        gateway_id: &str,
        cursor_seq: Option<u64>,
        acked_seq: Option<u64>,
    ) -> Result<runtime_recovery::RuntimeTransportState> {
        runtime_recovery::advance_gateway_transport_cursor(
            &self.conv_db,
            &self.session_id,
            gateway_id,
            cursor_seq,
            acked_seq,
        )
    }

    /// 读取当前会话网关 transport 状态。
    ///
    /// 参数:
    /// - `gateway_id`: 网关标识
    ///
    /// 返回:
    /// - transport 状态
    #[allow(dead_code)]
    pub(crate) fn load_gateway_transport_state(
        &self,
        gateway_id: &str,
    ) -> Result<Option<runtime_recovery::RuntimeTransportState>> {
        runtime_recovery::load_gateway_transport_state(&self.conv_db, &self.session_id, gateway_id)
    }

    /// 审计当前会话网关 transport 是否存在无法 replay 的未确认区间。
    ///
    /// 参数:
    /// - `gateway_id`: 网关标识
    ///
    /// 返回:
    /// - 是否写入恢复记录
    pub(crate) fn audit_gateway_transport_replay(&self, gateway_id: &str) -> Result<bool> {
        runtime_recovery::audit_gateway_transport_replay(
            &self.conv_db,
            &self.session_id,
            gateway_id,
        )
    }

    /// 写入当前会话网关 transport 事件到本地 replay source。
    ///
    /// 参数:
    /// - `gateway_id`: 网关标识
    /// - `sequence`: transport 事件序号
    /// - `payload`: 原始 Payload
    ///
    /// 返回:
    /// - 写入是否成功
    pub(crate) fn record_gateway_transport_event(
        &self,
        gateway_id: &str,
        sequence: u64,
        payload: &Value,
    ) -> Result<()> {
        runtime_recovery::record_gateway_transport_event(
            &self.conv_db,
            &self.session_id,
            gateway_id,
            sequence,
            payload,
        )?;
        Ok(())
    }

    /// 读取当前会话网关 transport 本地 replay 事件。
    ///
    /// 参数:
    /// - `gateway_id`: 网关标识
    /// - `start_sequence`: 起始序号
    /// - `end_sequence`: 结束序号
    ///
    /// 返回:
    /// - 按序排列的 transport 事件
    pub(crate) fn load_gateway_transport_events(
        &self,
        gateway_id: &str,
        start_sequence: i64,
        end_sequence: i64,
    ) -> Result<Vec<runtime_recovery::RuntimeTransportEvent>> {
        runtime_recovery::load_gateway_transport_events(
            &self.conv_db,
            &self.session_id,
            gateway_id,
            start_sequence,
            end_sequence,
        )
    }

    /// 开始应用当前会话网关 transport replay 事件。
    ///
    /// 参数:
    /// - `gateway_id`: 网关标识
    /// - `sequence`: transport 事件序号
    ///
    /// 返回:
    /// - replay 应用决策
    pub(crate) fn begin_gateway_transport_replay_event(
        &self,
        gateway_id: &str,
        sequence: u64,
    ) -> Result<runtime_recovery::RuntimeTransportReplayDecision> {
        runtime_recovery::begin_gateway_transport_replay_event(
            &self.conv_db,
            &self.session_id,
            gateway_id,
            sequence,
        )
    }

    /// 写入当前会话远端控制状态。
    ///
    /// 参数:
    /// - `state`: 待写入远端控制状态
    ///
    /// 返回:
    /// - 写入后的远端控制状态
    #[allow(dead_code)]
    pub(crate) fn upsert_remote_control_state(
        &self,
        state: RemoteControlStateUpsert,
    ) -> Result<RemoteControlState> {
        runtime_recovery::upsert_remote_control_state(&self.conv_db, state)
    }

    /// 读取当前会话远端控制状态。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 当前会话远端控制状态
    #[allow(dead_code)]
    pub(crate) fn load_remote_control_state(&self) -> Result<Option<RemoteControlState>> {
        runtime_recovery::load_remote_control_state(&self.conv_db, &self.session_id)
    }

    /// 推进当前会话远端控制游标和确认序号。
    ///
    /// 参数:
    /// - `subscribe_cursor`: 订阅游标
    /// - `server_seq`: 服务端最新序号
    /// - `acked_server_seq`: 已确认服务端序号
    ///
    /// 返回:
    /// - 推进后的远端控制状态
    #[allow(dead_code)]
    pub(crate) fn advance_remote_control_cursor(
        &self,
        subscribe_cursor: i64,
        server_seq: i64,
        acked_server_seq: i64,
    ) -> Result<RemoteControlState> {
        runtime_recovery::advance_remote_control_cursor(
            &self.conv_db,
            &self.session_id,
            subscribe_cursor,
            server_seq,
            acked_server_seq,
        )
    }

    /// 记录当前会话远端控制认证失败。
    ///
    /// 参数:
    /// - `reason`: 认证失败原因
    ///
    /// 返回:
    /// - 写入是否成功
    #[allow(dead_code)]
    pub(crate) fn record_remote_control_auth_failure(&self, reason: &str) -> Result<()> {
        runtime_recovery::record_remote_control_auth_failure(
            &self.conv_db,
            &self.session_id,
            reason,
        )
    }
}
