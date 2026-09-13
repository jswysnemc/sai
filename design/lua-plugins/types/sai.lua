---@meta

--- 宿主提供的公共接口子集，仅用于编辑器提示，不作为插件模块执行
---@class Sai
---@field plugin_id string 当前插件标识，修改它不会改变宿主归属
---@field config table<string, any> 当前插件自身设置，插件负责校验字段
---@field limits table<string, integer> 清单资源上限视图，修改它不会提高实际额度
---@field register_tool fun(definition: SaiToolDefinition): nil 初始化时注册工具
---@field register_command fun(definition: SaiCommandDefinition): nil 初始化时注册用户命令
---@field on fun(event: SaiEventName, callback: fun(event: table, ctx: SaiToolContext): any): nil 初始化时订阅普通生命周期事件
---@field json SaiJson JSON 值与文本转换
---@field text SaiText 有界纯文本处理
---@field crypto SaiCrypto 摘要计算
---@field time SaiTime 时间查询
---@field fs SaiFiles 授权文件读取
---@field storage SaiStorage 当前插件的数据存储
---@field http SaiHttp 授权文本请求
---@field binary SaiBinary 授权原始字节请求与缓冲
sai = {}
