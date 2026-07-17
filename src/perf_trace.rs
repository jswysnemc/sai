use std::time::Instant;

pub(crate) struct PerfTrace {
    enabled: bool,
    scope: &'static str,
    started_at: Instant,
    last_at: Instant,
}

impl PerfTrace {
    /// 创建性能监测器。
    ///
    /// 参数:
    /// - `scope`: 当前监测范围
    ///
    /// 返回:
    /// - 性能监测器，未启用时不会输出
    pub(crate) fn new(scope: &'static str) -> Self {
        let now = Instant::now();
        Self {
            enabled: std::env::var("SAI_PERF_TRACE").is_ok_and(|value| value != "0"),
            scope,
            started_at: now,
            last_at: now,
        }
    }

    /// 记录当前阶段耗时。
    ///
    /// 参数:
    /// - `stage`: 阶段名称
    pub(crate) fn mark(&mut self, stage: &str) {
        if !self.enabled {
            return;
        }
        let now = Instant::now();
        let delta = now.duration_since(self.last_at);
        let total = now.duration_since(self.started_at);
        self.last_at = now;
        eprintln!(
            "【性能监测】【{}】{} +{}ms total={}ms",
            self.scope,
            stage,
            delta.as_millis(),
            total.as_millis()
        );
    }
}
