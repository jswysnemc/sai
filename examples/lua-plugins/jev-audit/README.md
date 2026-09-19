# Jev 权限审核

该插件通过 TypeSafe 官方 Jev 的 `Choice` 原语审核 sai 的待审批工具操作，替换自动审核模式下的聊天模型审核。原生内核和 ACP 内核共用入口。默认模型为 `jev-latest`，地址为 `https://api.typesafe.ai/v1/systemone`。

## 安装

```sh
sai plugins check examples/lua-plugins/jev-audit
sai plugins install examples/lua-plugins/jev-audit
sai plugins enable jev-audit --grant-declared
```

安装默认禁用。启用命令授予清单声明的独立审核能力、TypeSafe 精确来源和推理 POST 端点，以及两个可选环境变量读取权限。插件没有工具执行、文件访问或存储写入能力。

## 供应商配置

复用 sai 的供应商凭据时，在主配置中增加专用于审核的供应商，并选择该插件：

```json
{
  "providers": [
    {
      "id": "typesafe",
      "display_name": "TypeSafe Jev",
      "base_url": "https://api.typesafe.ai/v1",
      "api_key": "$env:TYPESAFE_KEY",
      "models": ["jev-latest"],
      "default_model": "jev-latest",
      "enabled": false
    }
  ],
  "permission": {
    "auto_audit_plugin_id": "jev-audit",
    "auto_audit_provider_id": "typesafe",
    "auto_audit_model": "jev-latest"
  }
}
```

按字段合并到原配置，不要用上述片段覆盖其他供应商。`enabled: false` 避免把 Jev 放进聊天模型选单；审核插件仍可通过明确的供应商 ID 读取它。宿主只在内存中交付所选供应商的地址、模型和凭据，不把密钥复制到插件源码或插件配置。

在 TUI 使用 `/auto` 进入自动审核模式。其他入口使用原有 `auto_audit` 权限模式。恢复聊天模型审核时，一并清空上述三个自动审核字段即可使用会话模型；需要原专用审核模型时，恢复其供应商和模型字段。停用或撤销已选审核插件则交还人工，不隐式启用另一种自动审核。

不指定主配置中的供应商和模型时，插件也可读取 `TYPESAFE_KEY`，其次读取 `TYPESAFE_API_KEY`。独立设置使用：

```sh
sai plugins configure jev-audit examples/lua-plugins/jev-audit/settings.example.json
```

## 判断与失败处理

- 请求包含实际工具名、完整 JSON 参数、近期上下文、工作目录和 sai 原有审核规则。
- `Choice` 返回 `allow`、`deny` 或 `abstain`，同时带有全部选项概率和置信度。
- 默认仅在选项概率至少 `0.9` 且置信度至少 `0.8` 时自动提交允许或拒绝。阈值是可调整的初始策略，需要结合实际审核样本评估；置信度不等于决定正确率。
- `allow` 仅批准当前请求；`deny` 拒绝当前请求；弃权或置信不足时保持待人工处理。
- 缺少凭据、请求错误、超时、无效响应和不完整概率分布均交还人工。
- 人工先作出决定后，晚到的自动结果不能覆盖该决定。

`timeout_seconds` 可设置为 1–10 秒。`minimum_probability` 和 `minimum_confidence` 可设置为 0.5–1，默认值见设置样本。

接口参考：[TypeSafe HTTP API](https://docs.typesafe.ai/api)、[Choice](https://docs.typesafe.ai/primitives/choice)、[置信度](https://docs.typesafe.ai/confidence)。
