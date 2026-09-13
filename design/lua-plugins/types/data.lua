---@meta

--- JSON null 的不透明标记，只通过 sai.json.null 取得
---@class SaiJsonNull

--- JSON 表内的字段仍须可序列化；声明不代替宿主的递归校验
---@alias SaiJsonValue table|string|number|boolean|SaiJsonNull|nil

---@class SaiJson
---@field null SaiJsonNull 用于在对象中保留 JSON null 字段
---@field decode fun(text: string): SaiJsonValue 解析有界 JSON 文本
---@field encode fun(value: SaiJsonValue): string 将可序列化值编码为 JSON 文本
---@field array fun(value?: table): table 标记数组，空表也保留数组类型

---@class SaiText
---@field trim fun(text: string): string 去除两端 Unicode 空白
---@field lower fun(text: string): string 按 Unicode 规则转为小写
---@field collapse_whitespace fun(text: string): string 去除两端空白并合并内部连续空白
---@field clip fun(text: string, count: integer): string 按 Unicode 字符截取，必要时附加截断说明

---@class SaiCrypto
---@field digest fun(algorithm: string, text: string): string 返回指定算法的十六进制摘要，支持 sha256 等已公开算法

---@class SaiUtcTime
---@field unix_ms integer 同一次 UTC 时钟读取的 Unix 毫秒
---@field rfc3339 string 对应的完整 RFC 3339 文本

---@class SaiTime
---@field utc_now fun(): SaiUtcTime 返回同一时刻的整数毫秒与格式化文本
