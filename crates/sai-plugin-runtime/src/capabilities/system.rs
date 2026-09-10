use crate::manifest::validate_identifier;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// 【插件】【系统授权】文件根目录、环境变量和完整进程模板分别授权。
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct SystemCapabilities {
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub session_storage: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub workspace: bool,
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub read_paths: BTreeSet<String>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub environment: BTreeSet<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub processes: BTreeMap<String, ProcessTemplate>,
}

/// 【插件】【进程模板】固定程序与参数位置，变量只能替换整个参数，不经过 shell 展开。
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProcessTemplate {
    pub program: String,
    #[serde(default)]
    pub args: Vec<ProcessArgument>,
    #[serde(default = "empty_parameters")]
    pub parameters: Value,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub workspace: bool,
}

/// 【插件】【参数位置】区分固定字面量与 Schema 已声明的参数名称。
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ProcessArgument {
    Literal(String),
    Parameter(ProcessParameter),
}

/// 【插件】【参数引用】只允许引用一个完整参数，拒绝额外控制字段。
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProcessParameter {
    pub parameter: String,
}

impl SystemCapabilities {
    /// 【插件】【空授权】判断是否未声明任何系统能力。
    /// @returns 三类授权均为空时为 true
    pub fn is_empty(&self) -> bool {
        !self.session_storage
            && !self.workspace
            && self.read_paths.is_empty()
            && self.environment.is_empty()
            && self.processes.is_empty()
    }

    /// 【插件】【系统校验】验证数量、路径、环境变量及完整的进程参数契约。
    /// @returns 声明可交给宿主执行时成功
    pub fn validate(&self) -> Result<()> {
        if self.read_paths.len() > 64 || self.environment.len() > 64 || self.processes.len() > 64 {
            bail!("plugin system capabilities exceed 64 entries per category");
        }
        for path in &self.read_paths {
            validate_read_path(path)?;
        }
        for name in &self.environment {
            validate_environment_name(name)?;
        }
        for (name, process) in &self.processes {
            validate_identifier(name, 48).context("invalid process template name")?;
            process.validate()?;
        }
        Ok(())
    }

    /// 【插件】【系统交集】进程模板必须逐字段相同，修改模板不能继承旧授权。
    /// @param granted 用户明确授予的系统能力
    /// @returns 声明与授权共有的完整能力
    pub fn intersection(&self, granted: &Self) -> Self {
        Self {
            session_storage: self.session_storage && granted.session_storage,
            workspace: self.workspace && granted.workspace,
            read_paths: self
                .read_paths
                .intersection(&granted.read_paths)
                .cloned()
                .collect(),
            environment: self
                .environment
                .intersection(&granted.environment)
                .cloned()
                .collect(),
            processes: self
                .processes
                .iter()
                .filter(|(name, template)| granted.processes.get(*name) == Some(*template))
                .map(|(name, template)| (name.clone(), template.clone()))
                .collect(),
        }
    }

    /// 【插件】【系统范围】验证授权没有包含清单未声明的路径、变量或进程模板。
    /// @param declared 包清单中的系统能力
    /// @returns 当前授权是声明的子集时为 true
    pub fn is_subset(&self, declared: &Self) -> bool {
        (!self.session_storage || declared.session_storage)
            && (!self.workspace || declared.workspace)
            && self.read_paths.is_subset(&declared.read_paths)
            && self.environment.is_subset(&declared.environment)
            && self
                .processes
                .iter()
                .all(|(name, template)| declared.processes.get(name) == Some(template))
    }

    /// 【插件】【环境授权】环境读取只接受明确声明并授予的变量名称。
    /// @param name 要读取的环境变量
    /// @returns 允许访问时成功
    pub fn authorize_environment(&self, name: &str) -> Result<()> {
        validate_environment_name(name)?;
        if !self.environment.contains(name) {
            bail!("plugin environment variable is not allowed: {name}");
        }
        Ok(())
    }

    /// 【插件】【读取请求】先验证请求格式与能力存在性，真实路径与符号链接继续由宿主检查。
    /// @param path 要读取的文件或目录
    /// @returns 可进入宿主路径校验时成功
    pub fn check_read_request(&self, path: &str) -> Result<()> {
        validate_read_path(path)?;
        if self.read_paths.is_empty() {
            bail!("plugin file reading is not allowed");
        }
        Ok(())
    }

    /// 【插件】【进程授权】选择完整授权模板，并依据可信调用权限验证写入操作。
    /// @param name 模板名称；parameters 为调用参数；allow_writes 为宿主权限
    /// @returns 固定程序名与逐项展开后的 argv
    pub fn process_command(
        &self,
        name: &str,
        parameters: &Value,
        allow_writes: bool,
    ) -> Result<(String, Vec<String>)> {
        let template = self
            .processes
            .get(name)
            .context("plugin process template is not allowed")?;
        if !template.read_only && !allow_writes {
            bail!("read-only plugin callback cannot execute a writing process template");
        }
        template.command(parameters)
    }
}

impl ProcessTemplate {
    /// 【插件】【模板校验】程序路径不可变，参数变量必须属于对象 Schema 的显式字段。
    /// @returns 模板结构与参数 Schema 均有效时成功
    fn validate(&self) -> Result<()> {
        validate_argument(&self.program)?;
        if self.program.trim().is_empty()
            || self.program != self.program.trim()
            || self.program.starts_with('-')
            || self.args.len() > 64
        {
            bail!("invalid plugin process program or argument count");
        }
        crate::schema::compile_object(&self.parameters)?;
        if self.parameters.get("additionalProperties") != Some(&Value::Bool(false)) {
            bail!("process parameters must reject additional properties");
        }
        let fields = self.parameters.get("properties").and_then(Value::as_object);
        for argument in &self.args {
            match argument {
                ProcessArgument::Literal(value) => validate_argument(value)?,
                ProcessArgument::Parameter(reference) => {
                    validate_identifier(&reference.parameter, 48)?;
                    if !fields.is_some_and(|fields| fields.contains_key(&reference.parameter)) {
                        bail!("process argument references an undeclared parameter");
                    }
                }
            }
        }
        Ok(())
    }

    /// 【插件】【参数展开】将校验后的字符串、数字或布尔值放入单个 argv 位置。
    /// @param parameters 调用者提供的对象参数
    /// @returns 固定程序及不含 shell 拼接的参数列表
    fn command(&self, parameters: &Value) -> Result<(String, Vec<String>)> {
        if serde_json::to_vec(parameters)?.len() > 64 * 1024 {
            bail!("plugin process parameters exceed 64 KiB");
        }
        let validator = crate::schema::compile_object(&self.parameters)?;
        validator
            .validate(parameters)
            .map_err(|error| anyhow::anyhow!("invalid process parameters: {error}"))?;
        let mut arguments = Vec::with_capacity(self.args.len());
        for argument in &self.args {
            let value = match argument {
                ProcessArgument::Literal(value) => value.clone(),
                ProcessArgument::Parameter(reference) => match parameters.get(&reference.parameter)
                {
                    Some(Value::String(value)) => value.clone(),
                    Some(Value::Number(value)) => value.to_string(),
                    Some(Value::Bool(value)) => value.to_string(),
                    _ => bail!("process argument must reference a present scalar parameter"),
                },
            };
            validate_argument(&value)?;
            arguments.push(value);
        }
        if arguments.iter().map(String::len).sum::<usize>() > 64 * 1024 {
            bail!("plugin process arguments exceed 64 KiB");
        }
        Ok((self.program.clone(), arguments))
    }
}

/// 【插件】【路径声明】允许绝对路径、工作区相对路径和当前用户目录，不接受通配符或父目录跳转。
/// @param value 读取或写入声明及请求路径，名称中的波浪号保持字面含义
/// @returns 路径格式合法时成功，实际路径归属由宿主校验
pub fn validate_read_path(value: &str) -> Result<()> {
    // 1. 【插件】【用户目录】仅起始波浪号表示用户目录，Windows 短文件名中的波浪号不是展开语法
    if value.is_empty()
        || value.len() > 4096
        || value.chars().any(char::is_control)
        || value.contains(['*', '?', '<', '>', '|', '"'])
        || value.split(['/', '\\']).any(|part| part == "..")
        || (value.starts_with('~') && value != "~" && !value.starts_with("~/"))
    {
        bail!("plugin read path must be a bounded path without wildcards or parent traversal");
    }
    Ok(())
}

/// 【插件】【环境名称】限制环境变量标识，避免通配符和控制字符进入授权。
/// @param name 精确环境变量名称
/// @returns 名称可用于环境查询时成功
pub fn validate_environment_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        || name.as_bytes()[0].is_ascii_digit()
    {
        bail!("environment capability must be an exact variable name");
    }
    Ok(())
}

/// 【插件】【参数边界】拒绝操作系统不接受的 NUL 以及过长参数。
/// @param value 程序名称或单个参数
/// @returns 参数可作为独立 argv 元素时成功
fn validate_argument(value: &str) -> Result<()> {
    if value.len() > 8192 || value.contains('\0') {
        bail!("plugin process argument exceeds limits or contains NUL");
    }
    Ok(())
}

/// 【插件】【缺省参数】无变量的进程模板只接受空对象。
/// @returns 拒绝额外字段的对象 Schema
fn empty_parameters() -> Value {
    json!({"type":"object","additionalProperties":false})
}
