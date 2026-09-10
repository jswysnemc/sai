use crate::manifest::validate_identifier;
use crate::{EventKind, PluginCommand, PluginTool, ToolAccess};
use mlua::{Function, Lua, LuaSerdeExt, Table, Value};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub(super) struct RegisteredTool {
    pub definition: PluginTool,
    pub validator: Arc<jsonschema::Validator>,
    pub handler: Function,
}

pub(super) struct RegisteredCommand {
    pub definition: PluginCommand,
    pub handler: Function,
}

/// 【插件】【注册事务】加载完成后统一提交工具、命令和事件，不保留半成品。
#[derive(Default)]
pub(super) struct Registrations {
    pub closed: bool,
    pub tools: BTreeMap<String, RegisteredTool>,
    pub commands: BTreeMap<String, RegisteredCommand>,
    pub events: BTreeMap<EventKind, Vec<Function>>,
}

/// 【插件】【注册接口】向 Lua 安装工具、命令和事件注册入口。
/// @param lua 为虚拟机；api 为 sai 表；registrations 为加载事务
/// @returns 注册函数安装结果
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    registrations: Arc<Mutex<Registrations>>,
) -> mlua::Result<()> {
    let tools = registrations.clone();
    api.set(
        "register_tool",
        lua.create_function(move |lua, definition: Table| {
            let mut registrations = tools.lock().map_err(lua_error)?;
            ensure_open(&registrations)?;
            let name: String = definition.get("name")?;
            validate_identifier(&name, 48).map_err(lua_error)?;
            if registrations.tools.contains_key(&name) {
                return Err(mlua::Error::runtime(format!(
                    "duplicate plugin tool: {name}"
                )));
            }
            let description = description(&definition)?;
            let parameters: serde_json::Value = lua.from_value(definition.get("parameters")?)?;
            let validator = crate::schema::compile_object(&parameters)
                .map_err(|error| mlua::Error::runtime(format!("{error:#}")))?;
            let tool = RegisteredTool {
                definition: PluginTool {
                    name: name.clone(),
                    description,
                    parameters,
                    access: access(&definition)?,
                },
                validator: Arc::new(validator),
                handler: definition.get("execute")?,
            };
            registrations.tools.insert(name, tool);
            Ok(())
        })?,
    )?;

    let commands = registrations.clone();
    api.set(
        "register_command",
        lua.create_function(move |_, definition: Table| {
            let mut registrations = commands.lock().map_err(lua_error)?;
            ensure_open(&registrations)?;
            let name: String = definition.get("name")?;
            validate_identifier(&name, 48).map_err(lua_error)?;
            if registrations.commands.contains_key(&name) {
                return Err(mlua::Error::runtime(format!(
                    "duplicate plugin command: {name}"
                )));
            }
            let permission = access(&definition)?;
            if permission == ToolAccess::OptionalWrites {
                return Err(mlua::Error::runtime(
                    "optional_writes is only supported for tools",
                ));
            }
            let command = RegisteredCommand {
                definition: PluginCommand {
                    name: name.clone(),
                    description: description(&definition)?,
                    access: permission,
                },
                handler: definition.get("execute")?,
            };
            registrations.commands.insert(name, command);
            Ok(())
        })?,
    )?;

    api.set(
        "on",
        lua.create_function(move |_, (event, handler): (String, Function)| {
            let mut registrations = registrations.lock().map_err(lua_error)?;
            ensure_open(&registrations)?;
            let kind =
                serde_json::from_value(serde_json::Value::String(event)).map_err(lua_error)?;
            registrations.events.entry(kind).or_default().push(handler);
            Ok(())
        })?,
    )
}

/// 【插件】【注册校验】防止执行期间新增注册及加载时无限注册。
/// @param registrations 当前加载事务
/// @returns 事务未关闭且条目未超限时成功
fn ensure_open(registrations: &Registrations) -> mlua::Result<()> {
    let count = registrations.tools.len()
        + registrations.commands.len()
        + registrations.events.values().map(Vec::len).sum::<usize>();
    if registrations.closed || count >= 128 {
        return Err(mlua::Error::runtime(
            "plugin registrations are closed or exceed 128 entries",
        ));
    }
    Ok(())
}

/// 【插件】【说明校验】读取非空且长度受限的描述。
/// @param definition 注册定义
/// @returns 有效说明文字
fn description(definition: &Table) -> mlua::Result<String> {
    let value: String = definition.get("description")?;
    if value.trim().is_empty() || value.len() > 8192 {
        return Err(mlua::Error::runtime(
            "plugin description must contain 1-8192 bytes",
        ));
    }
    Ok(value)
}

/// 【插件】【权限声明】读取工具或命令的访问类型。
/// @param definition 注册定义
/// @returns 只读、写入或可选写入权限，未知声明返回错误
fn access(definition: &Table) -> mlua::Result<ToolAccess> {
    match definition.get::<Option<String>>("access")?.as_deref() {
        None | Some("read_only") => Ok(ToolAccess::ReadOnly),
        Some("writes") => Ok(ToolAccess::Writes),
        Some("optional_writes") => Ok(ToolAccess::OptionalWrites),
        Some(_) => Err(mlua::Error::runtime(
            "access must be read_only, writes or optional_writes",
        )),
    }
}

/// 【插件】【错误转换】将宿主错误转换为 Lua 可读错误。
/// @param error 原始错误
/// @returns Lua 运行错误
pub(super) fn lua_error(error: impl std::fmt::Display) -> mlua::Error {
    mlua::Error::runtime(error.to_string())
}

/// 【插件】【结果转换】保留字符串结果，将 Lua 表转换为 JSON 数据。
/// @param lua 虚拟机；value 为 Lua 返回值
/// @returns JSON 结果，无法序列化时返回错误
pub(super) fn json_value(lua: &Lua, value: Value) -> mlua::Result<serde_json::Value> {
    lua.from_value(value)
}
