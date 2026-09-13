# 图片生成与保存

调用 OpenAI 兼容图片生成接口，处理 Base64 或图片 URL 返回值，保存图片并可选择终端预览。供应商、密钥、输出目录均由本包独立设置提供。

## 安装、配置与运行

从仓库根目录执行；独立分发后将路径替换为自己的包目录。

```sh
sai plugins check examples/lua-plugins/image-generation
sai plugins pack examples/lua-plugins/image-generation --output ./image-generation-1.0.0.tar.gz
sai plugins install examples/lua-plugins/image-generation
sai plugins configure image-generation examples/lua-plugins/image-generation/settings.example.json
sai plugins enable image-generation --allow-http https://api.openai.com --allow-write-path '~/Pictures/sai/generated-images'
sai --yolo plugins call image-generation generate_image '{"prompt":"A small geometric landscape"}'
```

安装默认禁用且没有授权。模型使用的工具名称为 `lua__image-generation__generate_image`，其他入口也使用同样的包前缀。安装不会修改 Agent 白名单。配置样本不包含真实密钥或用户数据；旧主配置选项不再注入本包。

## 参数、权限与依赖

运行前在自己的设置文件填写有效 `api_keys`。本包不会读取旧主配置，也不展开环境引用。`provider_type` 支持原有 OpenAI 和 RightCode 请求策略；尺寸映射见 [sizes.lua](sizes.lua)。`timeout_seconds` 范围 1–600，Base64 与图片 URL 均受字节和总时长预算限制。URL 返回另需 `--allow-public-downloads`；下载不继承 API 凭据。

修改 `base_url` 或 `output_dir` 时，必须一起修改自己的清单和授权。保存设置不会扩大权限。`auto_print=false` 为独立包默认值；开启时可选安装 `image-display`，并对本包授予 `--allow-tool lua__image-display__print_image`。缺少显示包会跳过预览，预览失败仍返回已保存图片路径。生成操作需要写入权限。

配置仅保存业务设置，不调整能力声明或恢复撤权。具体字段见 [settings.example.json](settings.example.json) 和包内实现，公共权限说明见[示例索引](../README.md)。

## 更新、撤权与卸载

```sh
sai plugins install ./新版本目录 --replace
sai plugins disable image-generation
sai plugins remove image-generation
```

使用 `sai plugins enable image-generation` 的 `--no-http`、`--no-model`、`--no-tools`、`--no-processes`、`--no-file-write` 等对应选项撤销能力；只指定需要撤销的项，其余保持原值。普通重新启用不会恢复权限。替换安装保留设置与现有授权，新增声明须明确授权；已有会话通过 `/plugins reload` 使用新快照。

图片默认保存在 `~/Pictures/sai/generated-images`。卸载保留输出文件；旧版本源码可以重新安装，既有图片无需转换。 源码回退使用保留的旧发行包，与数据删除分开处理。

找不到工具时检查安装、启用状态和完整名称；权限错误检查 `sai plugins info image-generation --json`。缺少模型、系统工具、网络来源或可选插件时按错误信息修正对应依赖。验证使用隔离路径、本地服务和固定样本，不代表已经实测所有平台或外部供应商。

