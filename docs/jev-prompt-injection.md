# Jev 按需注入提示词与记忆

开启 `jev.routing.enabled` 后，系统提示词和指令文件中的 Jev 片段参与每轮请求前的判断。未命中时，本轮不追加片段正文；命中时，正文作为用户消息中的独立上下文块注入。工具和技能仍沿用现有暴露流程。

## 配置记忆

在 `config.jsonc` 中设置：

```json
{
  "jev": {
    "routing": {
      "enabled": true
    }
  },
  "memory": {
    "enabled": true,
    "association_enabled": true,
    "jev_injection": true
  }
}
```

也可在界面中设置：Web「设置 → Jev → 工具与 Skills 暴露决策」的「按需注入记忆」「按需加载标签提示词片段」两个开关，或 TUI `/config → Jev 与生图模型 → JEV → 工具与 Skills 暴露决策` 表单末尾的两项。两者都需要先开启暴露决策。

`memory.jev_injection` 默认 `false`。旧配置也可在 `plugins.memory` 下设置该字段；顶层 `memory` 非默认时按现有规则优先使用顶层配置。Jev 接入和密钥沿用现有 `model_endpoints` / `jev.endpoint_id` 配置。

- 路由生效且 `jev_injection=true`：记忆索引和记忆使用契约共用一个候选，命中后才注入。契约不进入静态系统提示。
- 路由关闭、或因 DeepSeek 锚定模式而不生效：保持原有索引与契约行为。
- `association_enabled=false`：不注入索引，仍可按需注入记忆使用契约。
- `memory.enabled=false` 或 `prompt_sections.memory_contract=false`：不创建记忆候选。
- 记忆工具不可用时不注入工具使用契约；索引仍遵守原有独立开关。

索引保留原有差异注入与去重逻辑；记忆正文仍通过记忆工具读取。Jev 获得候选的有限长度说明，不会自动获得全部记忆正文。

## 标记提示词片段

支持已加载的 `AGENT.md`、`AGENTS.md`、`CLAUDE.md`，以及配置中的系统提示、旧系统提示文件和调用方附加系统提示：

```xml
始终生效的常规规则。

<jev description="编写或修改数据库迁移时使用">
迁移必须提供回退方案，并验证现有数据兼容性。
</jev>

<jev>
撰写中文文档时使用项目约定的术语。
</jev>
```

该功能由 `jev.routing.prompt_segments` 控制，默认 `true`。设为 `false` 时片段按原文（含标签）随静态提示发送，不参与 Jev 判断；记忆注入不受影响。

有 `description` 时，用描述判断是否适用；没有描述或描述为空时，以正文作为候选说明。描述沿用现有候选长度限制，建议简短写明适用场景。正文命中后完整注入。支持多个片段、中文、单引号或双引号属性；不支持嵌套标签和其他属性。标签必须成对闭合，错误格式会报错，避免意外静态注入。

Jev 关闭时原文保持不变。Jev 开启时，指令文件后续更新和旧会话 baseline 也会过滤这些片段。附加提示在每轮刷新时保留，避免候选消失。

提示词每轮重新判断，按原文来源顺序注入，不占用工具或技能名额。命中片段随当前轮历史保存；之后未命中表示本轮不再追加，不会删除已经发送过的历史。压缩后再次命中会重新提供全文。Jev 请求失败、超时或答案缺失时不新增相关上下文，对话继续执行。

外部 ACP 内核使用相同判断，将命中的片段与记忆作为嵌入资源发送。

## 模块与验证

- `src/jev/prompt_segments.rs`：标签语法解析及静态内容分离。
- `src/jev/candidates.rs`、`selection.rs`：候选类型与概率选择。
- `src/agent/jev_prompt_context.rs`：每轮候选快照及注入渲染。
- `src/agent/jev_routing.rs`：复用一次 HTTP 请求评估工具、技能与提示词。
- `src/agent/system_prompt.rs`、`context_projection.rs`、`lifecycle.rs`：静态提示、指令更新及旧 baseline 过滤。
- `src/agent/turn_orchestration.rs`、`external_turn.rs`：将记忆判断应用到实际请求。

```sh
cargo check --tests --locked
cargo test --locked
```

测试覆盖解析边界、配置兼容、指令更新、旧 baseline、附加提示刷新、无工具模式、本地 HTTP 路由与完整聊天轮次中的实际请求内容。
