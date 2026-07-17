use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 网关进程记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GatewayProcessRecord {
    pub(crate) gateway_id: String,
    pub(crate) command: String,
    pub(crate) cwd: String,
    pub(crate) pid: u32,
    pub(crate) pgid: Option<i32>,
    pub(crate) status: String,
    pub(crate) stdout_log: String,
    pub(crate) stderr_log: String,
    pub(crate) started_at: u64,
    pub(crate) updated_at: u64,
}

impl GatewayProcessRecord {
    /// 判断记录是否处于运行状态。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 状态为 running 时返回 true
    pub(crate) fn is_running(&self) -> bool {
        self.status == "running"
    }

    /// 返回网关进程对应的运行时进程标识。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 运行时进程标识
    pub(crate) fn runtime_process_id(&self) -> String {
        format!("gateway_{}_{}", self.gateway_id, self.started_at)
    }
}

/// 网关进程状态存储，独立于通用后台命令存储。
#[derive(Debug, Clone)]
pub(crate) struct GatewayProcessStore {
    root: PathBuf,
}

impl GatewayProcessStore {
    /// 创建网关进程状态存储。
    ///
    /// 参数:
    /// - `state_dir`: Sai 状态目录
    ///
    /// 返回:
    /// - 网关进程状态存储
    pub(crate) fn new(state_dir: PathBuf) -> Self {
        Self {
            root: state_dir.join("gateways"),
        }
    }

    /// 初始化存储目录。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 初始化是否成功
    pub(crate) fn init(&self) -> Result<()> {
        std::fs::create_dir_all(self.logs_dir())?;
        Ok(())
    }

    /// 返回网关进程日志目录。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 日志目录路径
    pub(crate) fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// 返回进程记录文件路径。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 记录文件路径
    fn records_file(&self) -> PathBuf {
        self.root.join("processes.json")
    }

    /// 加载全部网关进程记录。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 网关进程记录列表
    pub(crate) fn load(&self) -> Result<Vec<GatewayProcessRecord>> {
        let path = self.records_file();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw = std::fs::read_to_string(&path)?;
        if raw.trim().is_empty() {
            return Ok(Vec::new());
        }
        Ok(serde_json::from_str(&raw)?)
    }

    /// 保存全部网关进程记录。
    ///
    /// 参数:
    /// - `records`: 网关进程记录列表
    ///
    /// 返回:
    /// - 保存是否成功
    pub(crate) fn save(&self, records: &[GatewayProcessRecord]) -> Result<()> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::write(self.records_file(), serde_json::to_vec_pretty(records)?)?;
        Ok(())
    }

    /// 用新的进程记录替换同网关旧记录。
    ///
    /// 参数:
    /// - `record`: 新网关进程记录
    ///
    /// 返回:
    /// - 替换是否成功
    pub(crate) fn replace_gateway_record(&self, record: GatewayProcessRecord) -> Result<()> {
        let mut records = self.load()?;
        records.retain(|item| item.gateway_id != record.gateway_id);
        records.push(record);
        self.save(&records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(gateway_id: &str, pid: u32) -> GatewayProcessRecord {
        GatewayProcessRecord {
            gateway_id: gateway_id.to_string(),
            command: "sai gateway qq-bot".to_string(),
            cwd: ".".to_string(),
            pid,
            pgid: None,
            status: "running".to_string(),
            stdout_log: "stdout.log".to_string(),
            stderr_log: "stderr.log".to_string(),
            started_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn replace_keeps_one_record_per_gateway() {
        let temp = tempfile::tempdir().unwrap();
        let store = GatewayProcessStore::new(temp.path().to_path_buf());
        store.init().unwrap();

        store.replace_gateway_record(record("qq", 100)).unwrap();
        store.replace_gateway_record(record("weixin", 101)).unwrap();
        store.replace_gateway_record(record("qq", 102)).unwrap();

        let records = store.load().unwrap();
        assert_eq!(records.len(), 2);
        let qq = records.iter().find(|item| item.gateway_id == "qq").unwrap();
        assert_eq!(qq.pid, 102);
    }

    #[test]
    fn load_returns_empty_when_file_missing() {
        let temp = tempfile::tempdir().unwrap();
        let store = GatewayProcessStore::new(temp.path().to_path_buf());

        assert!(store.load().unwrap().is_empty());
    }
}
