use super::*;

impl ReplRuntime {
    /// 在固定节流周期内刷新动效帧并冲刷待同步的流式内容。
    ///
    /// 下一次到期时间由上一次的计划时刻累加，而不是从本次实际唤醒时刻起算：
    /// 主循环 25ms 一跳、刷新间隔 32ms，两者不整除，按唤醒时刻累加会让每轮
    /// 都多等一个 tick，实际间隔被拉到 50ms。按计划时刻推进则只是对齐到
    /// 最近的 tick，长期平均仍是 32ms。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否执行了 live 刷新
    pub(in crate::cli) fn tick_live(&mut self) -> Result<bool> {
        let Some(next_refresh) = self.next_live_refresh else {
            return Ok(false);
        };
        let now = Instant::now();
        if now < next_refresh {
            return Ok(false);
        }
        let animated = self.transcript.advance_live_animation();
        let pending = std::mem::take(&mut self.live_sync_pending);
        if !animated && !pending {
            self.next_live_refresh = None;
            return Ok(false);
        }
        // 工作状态、reasoning 或未冲刷的正文仍在进行时保持节奏刷新；
        // 落后超过一个周期时（如终端卡顿）重新对齐到当前时刻，不追补欠帧
        let planned = next_refresh + LIVE_REFRESH_INTERVAL;
        self.next_live_refresh = Some(if planned > now {
            planned
        } else {
            now + LIVE_REFRESH_INTERVAL
        });
        self.sync_transcript(true)?;
        Ok(true)
    }

    /// 刷新后台子智能体的持久化时间线。
    ///
    /// 返回:
    /// - 是否执行了 transcript 同步
    pub(super) fn tick_subagents(&mut self) -> Result<bool> {
        let background = self.tick_background_logs()?;
        let signature = self.transcript.subagent_signature();
        if signature == self.subagent_signature && !background {
            return Ok(false);
        }
        self.subagent_signature = signature;
        self.transcript.mark_subagents_dirty();
        self.sync_transcript(true)?;
        Ok(true)
    }

    /// 处理输入阶段的定时重绘。
    ///
    /// 返回:
    /// - 是否执行了任何刷新
    pub(in crate::cli) fn process_idle_tick(&mut self) -> Result<bool> {
        let reflowed = self.maybe_reflow_due(false)?;
        let subagents = self.tick_subagents()?;
        // 观察者模式下主循环卡在读键上，远端事件只能借空闲节拍落地
        let followed = self.drain_follow_events()?;
        // 停留在运行中的子智能体视图，或后台仍有子智能体运行（底部面板
        // 的流光与实时统计）时，空闲期也要驱动 live 刷新
        if self.next_live_refresh.is_none()
            && (self.transcript.viewing_running_subagent()
                || self.transcript.has_running_subagents()
                || self.transcript.has_running_background_commands())
        {
            self.next_live_refresh = Some(Instant::now());
        }
        let animated = self.tick_live()?;
        Ok(reflowed || subagents || followed || animated)
    }
}
