local decode = require("decode")
local result = require("result")
local M = {}

local supported = {
    md5 = true, sha1 = true, sha224 = true, sha256 = true, sha384 = true, sha512 = true,
    sha3_224 = true, sha3_256 = true, sha3_384 = true, sha3_512 = true,
    blake2b = true, blake2s = true, blake3 = true, crc32 = true, adler32 = true,
}
local aliases = {
    ["sha3-224"] = "sha3_224", ["sha3-256"] = "sha3_256",
    ["sha3-384"] = "sha3_384", ["sha3-512"] = "sha3_512", b2sum = "blake2b",
}
local mainstream = {
    "md5", "sha1", "sha224", "sha256", "sha384", "sha512",
    "sha3_224", "sha3_256", "sha3_384", "sha3_512",
    "blake2b", "blake2s", "b2sum", "crc32", "adler32",
}

--- 【哈希工具】【算法选择】保留默认列表与逗号、ASCII 空格分隔规则
--- @param value string 原始算法选择参数
--- @return table 算法名称列表，保留大小写、别名及重复项
local function select_algorithms(value)
    if sai.text.trim(value) == "" or value == "all" or value == "mainstream" then
        return mainstream
    end
    local names = {}
    for name in value:gmatch("[^, ]+") do names[#names + 1] = name end
    return names
end

--- 【哈希工具】【摘要计算】将输入还原为字节，按插件规则选择算法并组装兼容结果
--- @param args table 含 input_text、可选 input_format 及 algorithms 的工具参数
--- @return string 含 success、byte_length 和 results 的 JSON 文本
function M.run(args)
    -- 1. 【哈希工具】【输入解析】二进制输入保留原始字节，不先转换为 UTF-8
    local bytes = decode.bytes(args.input_text, args.input_format or "text")
    local results = {}
    local names = {}
    -- 2. 【哈希工具】【算法派发】错误算法作为单项结果保留，不阻断其余摘要
    for _, name in ipairs(select_algorithms(args.algorithms or "sha256")) do
        if results[name] == nil then names[#names + 1] = name end
        local normalized = sai.text.lower(name)
        local canonical = aliases[normalized] or normalized
        if supported[canonical] then
            results[name] = sai.crypto.digest(canonical, bytes)
        else
            results[name] = "unsupported algorithm: " .. normalized
        end
    end
    return result.hashes(#bytes, names, results)
end

return M
