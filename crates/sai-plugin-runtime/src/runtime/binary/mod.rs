mod buffer;
mod conditional;
mod constructors;
mod document;
mod files;
mod inspection;
mod json;
mod network;
mod sqlite;
mod sqlite_request;
mod terminal;
mod vision;

use super::control::CallControl;
use crate::host::{binary::BinaryBudget, PluginHost};
use crate::{Capabilities, ExecutionLimits};
use mlua::{Lua, Table};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// 【插件二进制】【共享约束】所有二进制入口共用 VM 预算和可信调用状态。
struct BinaryServices {
    host: Arc<dyn PluginHost>,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
    budget: Arc<BinaryBudget>,
}

impl BinaryServices {
    /// 【插件二进制】【调用代次】拒绝加载阶段和回调结束后的宿主操作。
    /// @returns 当前回调的代次，用于绑定缓冲生命周期
    fn generation(&self) -> mlua::Result<u64> {
        self.control.system_context()?;
        Ok(self.control.generation.load(Ordering::Acquire))
    }

    /// 【插件二进制】【代次验证】缓冲只能在创建它的回调内使用。
    /// @param generation 缓冲创建时的代次
    /// @returns 当前回调仍然匹配时成功
    fn check(&self, generation: u64) -> mlua::Result<()> {
        if self.generation()? != generation {
            return Err(mlua::Error::runtime(
                "binary buffer belongs to an expired callback",
            ));
        }
        Ok(())
    }

    /// 【插件二进制】【调用计数】为有副作用的宿主操作消耗一次系统额度。
    /// @returns 未超过本回调额度时成功
    fn charge(&self) -> mlua::Result<()> {
        super::system::charge(&self.control, self.limits.system_calls)
    }
}

/// 【插件二进制】【入口安装】将大正文保存在不参与 JSON 工具输出的受控句柄内。
/// @param lua 虚拟机；api 为 sai 表；host 为宿主；capabilities 为授权；limits 为限制；control 为状态
/// @returns 网络、文件读取、解码和终端入口的安装结果
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    host: Arc<dyn PluginHost>,
    capabilities: Capabilities,
    limits: ExecutionLimits,
    control: Arc<CallControl>,
) -> mlua::Result<()> {
    let budget = BinaryBudget::new(limits.binary_bytes);
    let services = Arc::new(BinaryServices {
        host,
        capabilities,
        limits,
        control,
        budget,
    });
    // 1. 【插件二进制】【元表初始化】在 coroutine 全局入口封闭前注册异步句柄方法，不执行宿主 I/O
    let empty = services
        .budget
        .retain(Vec::new())
        .map_err(super::system::error)?;
    lua.create_userdata(buffer::Buffer::new(empty, services.clone(), 0)?)?;
    let binary = lua.create_table()?;
    network::install(lua, &binary, services.clone())?;
    files::install(lua, &binary, services.clone())?;
    constructors::install(lua, &binary, services.clone())?;
    api.set("binary", binary)?;
    sqlite::install(lua, api, services.clone())?;
    vision::install(lua, api, services.clone())?;
    terminal::install(lua, api, services)
}
