# Lua 玄学与骰子

内置 `xuanxue` 包提供周易起卦、塔罗抽牌、吉凶抽签和掷骰子。四个公开工具名称、原参数 Schema、卦牌签文和骰子结果字段保持兼容；业务由 Lua 实现，清单显式声明 `capabilities: {}`。

## 包与工具

```text
plugins/xuanxue/
├── sai-plugin.json   # 包信息、空能力声明与限额
├── init.lua          # 四个只读工具的注册契约
├── draw.lua          # 卦象、牌名、正逆位和签文抽取
├── dice.lua          # 参数默认值、收窄、点数与溢出判断
├── render.lua        # 原顺序的骰子 JSON
└── data/
    ├── zhouyi.lua    # 64 个卦名
    ├── tarot.lua     # 78 张塔罗牌
    └── fortune.lua   # 7 组等级与含义
```

| 工具 | 参数 | 返回值 |
| --- | --- | --- |
| `draw_zhouyi_hexagram` | 空对象 | 原卦名文本 |
| `draw_tarot_card` | 空对象 | `牌名（正位）` 或 `牌名（逆位）` |
| `draw_fortune_lot` | 空对象 | `等级：含义` |
| `roll_dice` | 可选整数 `count`、`sides`、`modifier` | 紧凑 JSON，字段顺序为 `ok`、`count`、`sides`、`rolls`、`total`、`modifier`、`modified_total` |

抽取使用 Lua 5.4 的 `math.random`，每个闭区间均匀选择。塔罗牌与正逆位分别抽取。随机状态属于插件 VM，同一实例的克隆共享状态，新实例独立；不保证与旧 Rust 随机源产生相同序列。公开参数没有种子或固定选择选项。

## 骰子数字契约

`count` 默认 1，非负整数收窄到 1–100；`sides` 默认 6，非负整数收窄到 2–1000。负整数沿用旧默认值，所以 `count=-1` 得到 1，`sides=-1` 得到 6。零会分别收窄到 1 和 2。

保留原 `serde_json` 数字表示行为：JSON Schema 接受数学上的整数，但业务仍区分整数表示与浮点表示。

| 输入示例 | 行为 |
| --- | --- |
| `{"count":3}` | 投掷 3 颗骰子 |
| `{"count":3.0}` 或 `{"count":3e0}` | 浮点表示采用默认 1 颗 |
| `{"count":18446744073709551615}` | 无符号 64 位整数收窄到 100 颗 |
| `{"count":18446744073709551616}` | JSON 解析为浮点值，采用默认 1 颗 |
| `{"sides":0}` | 每颗骰子 2 面 |
| `{"modifier":-9223372036854775808}` | 精确保留有符号 64 位最小值 |
| `{"modifier":9223372036854775808}` 或 `{"modifier":3.0}` | 不属于原有符号整数范围或表示，采用默认 0 |

Lua 无法单靠转换后的浮点值区分超大无符号整数与浮点输入。`dice.lua` 通过通用 `ctx.json_integer(pointer)` 查询原始整数文本，再收窄数量和面数。该接口只读取当前输入，不需要宿主能力授权，详见 [Lua API](api.md)。

每颗骰子结果为 1–`sides` 的整数，`total` 是全部点数之和。计算 `modified_total` 前检查有符号 64 位上界；实际总和溢出时返回 `modified dice total exceeds signed 64-bit range`。旧实现会在调试构建中 panic，在发布构建中回绕；迁移版统一返回错误，后续调用仍可继续。

原生直接执行入口没有始终校验工具 Schema，因此部分错误类型会采用默认值。Lua 入口统一执行原 Schema：非对象、额外字段、字符串数字、布尔值、null、数组以及非整数小数均在随机抽取前拒绝；缺失的可选字段继续采用默认值。

## 设置与资源

旧主配置 `plugins.xuanxue.enabled` 提供缺省启用值，`plugins.jsonc` 中显式的 `xuanxue.enabled` 优先。普通和只读注册表均按有效开关提供四个工具；禁用后，设置目录仍保留工具条目供白名单配置使用。

```sh
sai plugins info xuanxue --json
sai plugins disable xuanxue
sai plugins enable xuanxue
```

包没有额外设置、用户命令或事件监听器，也没有外部能力。宿主额外授予网络、模型、工具或系统能力，不会扩大空清单的有效权限。

| 资源 | 发布上限 |
| --- | --- |
| Lua 堆 | 1 MiB |
| 单次指令及原生计算预算 | 100000 |
| 单次回调时长 | 1 秒 |
| JSON 输入、输出及最终结果 | 4096 字节 |

原始工具参数由共用 JSON 与 Schema 入口处理；上表的 JSON 输入字节上限适用于 `sai.json.decode`，并非整个工具参数文本的通用长度上限。骰子数量和面数始终由业务收窄，最多产生 100 个点数。

## 验证范围

`src/plugins/tests/fixtures/xuanxue_draws.json` 和 `xuanxue_dice.json` 冻结提交 `5cbaad0` 的 330 组样本。参考程序保留原函数，仅替换随机源，并使用应用相同的依赖特性，包括 `serde_json/preserve_order`。

- 301 组合法且未溢出的结果逐字节一致，包含全部 64 卦、78 张牌的两个方向、7 组签文及 74 组骰子边界；每次随机请求的区间和次数也与原版相同。
- 26 组违反 Schema 的输入在抽取前拒绝；3 组原版溢出输入返回明确错误。
- 测试直接加载发布用 Lua 包，覆盖旧开关、显式覆盖、目录与过滤、九类外部能力拒绝、实例随机状态、最大输出、指令限制及超时恢复。
- 运行时独立验证原整数表示、64 位边界、嵌套路径和转义、输入快照、查询失效、非法路径及共用预算。

发布程序与完整回归结果见[迁移记录](migration.md)。
