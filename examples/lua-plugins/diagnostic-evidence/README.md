# 系统诊断证据

采集 Linux 多领域证据与 macOS 基础证据，区分只读取证和显式应用执行。Windows 自动识别会返回平台不支持结果。

## 安装、配置与运行

从仓库根目录执行；独立分发后将路径替换为自己的包目录。

```sh
sai plugins check examples/lua-plugins/diagnostic-evidence
sai plugins pack examples/lua-plugins/diagnostic-evidence --output ./diagnostic-evidence-1.0.0.tar.gz
sai plugins install examples/lua-plugins/diagnostic-evidence
sai plugins configure diagnostic-evidence examples/lua-plugins/diagnostic-evidence/settings.example.json
sai plugins enable diagnostic-evidence --allow-read-path /etc/os-release --allow-process system-info
sai --plan plugins call diagnostic-evidence check_issue '{"area":"system","depth":"quick"}'
```

安装默认禁用且没有授权。模型使用的工具名称为 `lua__diagnostic-evidence__check_issue`，其他入口也使用同样的包前缀。安装不会修改 Agent 白名单。配置样本不包含真实密钥或用户数据；旧主配置选项不再注入本包。

## 参数、权限与依赖

`area` 支持 `system`、`app`、`input_method`、`display`、`audio`、`package`、`package_update`、`gpu`、`network`、`storage` 或 `auto`。自动模式需要 `query`；`target` 可指定应用或子系统。未授权或缺少系统命令的证据项会明确记录失败。

以上最小授权用于系统基本信息。完整取证按 [清单](sai-plugin.json) 选择读取目录、环境变量和只读进程模板；不需要网络或模型。`diagnostic_app_probe` 执行 `--version` 或启动应用，必须使用允许写入的模式，版本探测另需 `app-version` 模板，启动探测另需 `--allow-tool run_command`。基础取证不会隐式执行目标应用。`command_timeout_ms` 范围 1–120000，输出上限由独立设置与宿主预算共同约束。

配置仅保存业务设置，不调整能力声明或恢复撤权。具体字段见 [settings.example.json](settings.example.json) 和包内实现，公共权限说明见[示例索引](../README.md)。

## 更新、撤权与卸载

```sh
sai plugins install ./新版本目录 --replace
sai plugins disable diagnostic-evidence
sai plugins remove diagnostic-evidence
```

使用 `sai plugins enable diagnostic-evidence` 的 `--no-http`、`--no-model`、`--no-tools`、`--no-processes`、`--no-file-write` 等对应选项撤销能力；只指定需要撤销的项，其余保持原值。普通重新启用不会恢复权限。替换安装保留设置与现有授权，新增声明须明确授权；已有会话通过 `/plugins reload` 使用新快照。

不保存业务数据。 源码回退使用保留的旧发行包，与数据删除分开处理。

找不到工具时检查安装、启用状态和完整名称；权限错误检查 `sai plugins info diagnostic-evidence --json`。缺少模型、系统工具、网络来源或可选插件时按错误信息修正对应依赖。验证使用隔离路径、本地服务和固定样本，不代表已经实测所有平台或外部供应商。

