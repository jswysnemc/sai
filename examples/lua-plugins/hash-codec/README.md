# 摘要与文本解码

本示例计算文本或二进制输入的摘要，并解码 Base64、十六进制、URL、HTML 与 ROT13 文本。使用公共 Lua 编码接口，无网络、文件、进程、模型或存储能力，不依赖其他插件。

## 安装与运行

以下命令从 sai 仓库根目录执行，也可将本包复制到任意位置后替换源码路径：

```sh
sai plugins check examples/lua-plugins/hash-codec
sai plugins pack examples/lua-plugins/hash-codec --output ./hash-codec-1.0.0.tar.gz
sai plugins install examples/lua-plugins/hash-codec
sai plugins configure hash-codec examples/lua-plugins/hash-codec/settings.example.json
sai plugins enable hash-codec
sai --plan plugins call hash-codec calculate_hash '{"input_text":"abc","algorithms":"sha256"}'
sai --plan plugins call hash-codec decode_encoded_text '{"input_text":"SGVsbG8=","input_format":"base64"}'
```

第一个调用返回包含 `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad` 的 JSON；第二个返回解码结果 `Hello`。`check` 应报告两个工具和零个命令。无需配置模型，支持 sai Lua 运行时可用的平台。

模型工具名称是 `lua__hash-codec__calculate_hash` 和 `lua__hash-codec__decode_encoded_text`。使用 Agent 白名单时显式选择这两个名称。旧 `plugins.hash_codec` 开关与短名称不再生效。

## 参数、配置与限制

`calculate_hash` 必须提供 `input_text`，可指定 `algorithms` 和 `input_format`（`text`、`hex`、`base64`）。支持 MD5、SHA 系列、BLAKE2、BLAKE3、CRC32、Adler32 及 `all`、`mainstream` 组合；`b2sum` 对应 BLAKE2b-512。与 shell 的 `echo` 对照时须明确是否包含结尾换行。

`decode_encoded_text` 必须提供 `input_text` 和 `input_format`（`base64`、`hex`、`url`、`html`、`rot13`）。`text_encoding` 保留旧参数兼容性，仍按原有 UTF-8 行为处理。

此包没有业务设置，示例配置为 `{}`。参数类型、未知字段和枚举由工具 Schema 校验；输入、输出与计算量继续受到清单预算限制。格式错误和超限返回明确错误，后续正常调用可继续执行。

## 更新与卸载

更新版本后检查源码，再使用 `sai plugins install ./新版本目录 --replace`。新调用加载更新快照，现有会话执行 `/plugins reload`。本包没有可撤销的外部能力，也不保存用户数据。

```sh
sai plugins disable hash-codec
sai plugins remove hash-codec
```

找不到工具时检查安装状态、启用状态和实际完整名称。若需要回退，重新安装保留的旧源码并使用 `--replace`，无需回退数据。

`init.lua` 负责注册和参数契约，`hashes.lua` 负责摘要，`decode.lua` 负责解码，`result.lua` 负责结果格式。业务回归保留 175 组冻结结果，以及资源超限和恢复验证。
