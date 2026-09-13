# 输入法问题调查

通过当前会话选定的模型组织多轮输入法调查，生成结构化报告和用量统计。模型服务、工具调度、预算及取消由公共宿主提供。

## 安装、配置与运行

从仓库根目录执行；独立分发后将路径替换为自己的包目录。

```sh
sai plugins check examples/lua-plugins/input-method-investigation
sai plugins pack examples/lua-plugins/input-method-investigation --output ./input-method-investigation-1.0.0.tar.gz
sai plugins install examples/lua-plugins/input-method-investigation
sai plugins configure input-method-investigation examples/lua-plugins/input-method-investigation/settings.example.json
sai plugins enable input-method-investigation --allow-model --allow-tool check_os_info --allow-tool lua__diagnostic-evidence__check_issue
sai --plan plugins call input-method-investigation linux_input_method_diagnose '{"issue":"Wayland 下无法输入中文","target":"firefox"}'
```

安装默认禁用且没有授权。模型使用的工具名称为 `lua__input-method-investigation__linux_input_method_diagnose`，其他入口也使用同样的包前缀。安装不会修改 Agent 白名单。配置样本不包含真实密钥或用户数据；旧主配置选项不再注入本包。

## 参数、权限与依赖

运行前须配置宿主模型，并安装、启用且授权实际需要的资料工具。推荐安装 `diagnostic-evidence` 与 `fcitx-wiki`；可选 `web-search`、`web-fetch`、`knowledge-base`。每项外部调用都需要完整工具名称授权，并受当前 Agent 白名单限制。包清单列出了允许选择的依赖，不会自动安装或启用它们。

`issue` 必填，`target` 可选。`max_tool_steps` 为非负整数，0 表示由宿主预算限制；`tool_timeout_ms` 为正整数，仍受宿主期限约束。`progress_mode` 为 `hidden`、`summary` 或 `full`。缺少可选资料时模型只使用实际可用工具；模型撤权则请求失败。工具超时会取消该调用并记录失败，调查可继续整理已有证据。

配置仅保存业务设置，不调整能力声明或恢复撤权。具体字段见 [settings.example.json](settings.example.json) 和包内实现，公共权限说明见[示例索引](../README.md)。

## 更新、撤权与卸载

```sh
sai plugins install ./新版本目录 --replace
sai plugins disable input-method-investigation
sai plugins remove input-method-investigation
```

使用 `sai plugins enable input-method-investigation` 的 `--no-http`、`--no-model`、`--no-tools`、`--no-processes`、`--no-file-write` 等对应选项撤销能力；只指定需要撤销的项，其余保持原值。普通重新启用不会恢复权限。替换安装保留设置与现有授权，新增声明须明确授权；已有会话通过 `/plugins reload` 使用新快照。

本包不保存额外业务数据，模型对话和工具事件仍属于宿主会话记录。 源码回退使用保留的旧发行包，与数据删除分开处理。

找不到工具时检查安装、启用状态和完整名称；权限错误检查 `sai plugins info input-method-investigation --json`。缺少模型、系统工具、网络来源或可选插件时按错误信息修正对应依赖。验证使用隔离路径、本地服务和固定样本，不代表已经实测所有平台或外部供应商。

