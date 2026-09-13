---@meta

---@class SaiReadTextOptions
---@field max_bytes? integer 默认 1 MiB，收窄到至少 1 字节及回调输出上限
---@field lossy? boolean 默认 false；true 使用 UTF-8 替换解码

---@class SaiTextFile
---@field text string 有界 UTF-8 正文
---@field truncated boolean 是否还有未返回的正文

---@class SaiFiles
---@field read_text fun(path: string, options?: SaiReadTextOptions): SaiTextFile 按 system.read_paths 授权读取普通文件，相对路径基于任务目录
---@field realpath fun(path: string): string 返回已授权且存在的普通文件或目录的规范绝对路径
