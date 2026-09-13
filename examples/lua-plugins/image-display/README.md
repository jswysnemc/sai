# 终端图片显示

使用公共终端展示服务绘制本地图片，提供尺寸优先级、终端百分比和中文/英文反馈。不依赖其他插件、模型或网络。

## 安装、配置与运行

从仓库根目录执行；独立分发后将路径替换为自己的包目录。

```sh
sai plugins check examples/lua-plugins/image-display
sai plugins pack examples/lua-plugins/image-display --output ./image-display-1.0.0.tar.gz
sai plugins install examples/lua-plugins/image-display
sai plugins configure image-display examples/lua-plugins/image-display/settings.example.json
sai plugins enable image-display --allow-image-display
sai --plan plugins call image-display print_image '{"image":"./picture.png","width":60}'
```

安装默认禁用且没有授权。模型使用的工具名称为 `lua__image-display__print_image`，其他入口也使用同样的包前缀。安装不会修改 Agent 白名单。配置样本不包含真实密钥或用户数据；旧主配置选项不再注入本包。

## 参数、权限与依赖

`image` 必填。显式 `width`/`height` 优先，其次 `size`（例如 `60x20`），最后使用终端百分比；缺少终端尺寸时交给宿主默认渲染。实际宽高分别限制在 300 列、200 行。`language` 为 `zh` 或 `en`。支持宿主终端图片协议或其 ANSI 降级输出的平台；非交互环境可能无法预览。

生成和搜图示例通过 `lua__image-display__print_image` 调用本包。安装显示包不会自动给调用方授予工具调用能力；调用方白名单也必须允许该完整名称。

配置仅保存业务设置，不调整能力声明或恢复撤权。具体字段见 [settings.example.json](settings.example.json) 和包内实现，公共权限说明见[示例索引](../README.md)。

## 更新、撤权与卸载

```sh
sai plugins install ./新版本目录 --replace
sai plugins disable image-display
sai plugins remove image-display
```

使用 `sai plugins enable image-display` 的 `--no-http`、`--no-model`、`--no-tools`、`--no-processes`、`--no-file-write` 等对应选项撤销能力；只指定需要撤销的项，其余保持原值。普通重新启用不会恢复权限。替换安装保留设置与现有授权，新增声明须明确授权；已有会话通过 `/plugins reload` 使用新快照。

不保存业务数据，卸载不会删除传入的图片。 源码回退使用保留的旧发行包，与数据删除分开处理。

找不到工具时检查安装、启用状态和完整名称；权限错误检查 `sai plugins info image-display --json`。缺少模型、系统工具、网络来源或可选插件时按错误信息修正对应依赖。验证使用隔离路径、本地服务和固定样本，不代表已经实测所有平台或外部供应商。

