use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 【插件】【系统上下文】由宿主绑定真实工作目录，Lua 不能覆盖该值。
#[derive(Clone, Debug, Default)]
pub struct SystemContext {
    pub workdir: String,
    pub allow_writes: bool,
}

/// 【插件】【文件读取】限制读取字节数，显式选择非法 UTF-8 的处理方式。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileReadRequest {
    pub path: String,
    pub max_bytes: usize,
    pub lossy: bool,
}

/// 【插件】【文件正文】保留截断信息，调用方自行决定缺失内容是否影响业务证据。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FileText {
    pub text: String,
    pub truncated: bool,
}

/// 【插件】【文件属性】仅公开通用文件类型和字节数。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FileInfo {
    pub is_file: bool,
    pub is_dir: bool,
    pub len: u64,
}

/// 【插件】【目录条目】返回名称和路径，目录读取不隐式读取子项正文。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_file: bool,
}

/// 【插件】【目录结果】通过截断标记区分空目录和读取条数上限。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DirectoryListing {
    pub entries: Vec<DirectoryEntry>,
    pub truncated: bool,
}

/// 【插件】【进程请求】只选择已授权模板，不能指定新的程序、工作目录或环境覆盖。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessRequest {
    pub template: String,
    pub parameters: Value,
    pub timeout_ms: u64,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
}

/// 【插件】【进程结果】分别记录退出状态、超时和两路输出截断。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProcessOutput {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}
