local hashes = require("hashes")
local decode = require("decode")

-- 1. 【哈希工具】【注册契约】保留原工具名称、参数及只读权限
sai.register_tool({
    name = "calculate_hash",
    description = "Calculate hashes for text/hex/base64 input. Supports md5, sha1, sha224, sha256, sha384, sha512, sha3_224, sha3_256, sha3_384, sha3_512, blake2b, b2sum, blake2s, blake3, crc32, adler32, and all/mainstream. b2sum matches GNU/coreutils b2sum: BLAKE2b-512. For shell echo semantics include the trailing newline in input_text.",
    access = "read_only",
    parameters = {
        type = "object",
        properties = {
            input_text = {type = "string", description = "Input text bytes. Include \\n when matching shell commands like echo."},
            algorithms = {type = "string"},
            input_format = {type = "string", enum = sai.json.array({"text", "hex", "base64"})},
        },
        required = sai.json.array({"input_text"}),
        additionalProperties = false,
    },
    execute = hashes.run,
})

-- 2. 【文本解码】【注册契约】text_encoding 保留为兼容字段，沿用原有 UTF-8 解码行为
sai.register_tool({
    name = "decode_encoded_text",
    description = "Decode base64, hex, url, html, or rot13 encoded text.",
    access = "read_only",
    parameters = {
        type = "object",
        properties = {
            input_text = {type = "string"},
            input_format = {type = "string", enum = sai.json.array({"base64", "hex", "url", "html", "rot13"})},
            text_encoding = {type = "string"},
        },
        required = sai.json.array({"input_text", "input_format"}),
        additionalProperties = false,
    },
    execute = decode.run,
})
