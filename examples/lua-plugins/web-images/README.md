# 网页图片搜索与筛选

查询 DuckDuckGo 与 Bing，排序去重后返回远程元数据；允许写入时可下载图片、进行可选视觉筛选和预览。

## 安装、配置与运行

从仓库根目录执行；独立分发后将路径替换为自己的包目录。

```sh
sai plugins check examples/lua-plugins/web-images
sai plugins pack examples/lua-plugins/web-images --output ./web-images-1.0.0.tar.gz
sai plugins install examples/lua-plugins/web-images
sai plugins configure web-images examples/lua-plugins/web-images/settings.example.json
sai plugins enable web-images --allow-http https://duckduckgo.com --allow-http https://www.bing.com
sai --plan plugins call web-images search_web_images '{"query":"mountain landscape","count":1,"preview":false}'
```

安装默认禁用且没有授权。模型使用的工具名称为 `lua__web-images__search_web_images`，其他入口也使用同样的包前缀。安装不会修改 Agent 白名单。配置样本不包含真实密钥或用户数据；旧主配置选项不再注入本包。

## 参数、权限与依赖

`query` 和 `count` 必填，数量以用户请求为准，上限受设置和宿主共同约束。以上只读运行返回远程元数据，不下载图片。下载另需 `--allow-public-downloads --allow-write-path '~/Pictures/sai/web-images'` 与允许写入的执行模式；公开下载遵守宿主对网络地址的限制。

视觉筛选需开启 `vision_screening_enabled` 并授予 `--allow-vision`，使用宿主选择的视觉服务。预览是可选依赖：安装 `image-display`、授予本包 `--allow-tool lua__image-display__print_image`，再设置 `auto_preview=true` 或传入 `preview=true`。缺少显示包不影响搜索或保存；视觉服务失败会记录状态并保留已有图片。修改搜索地址或缓存目录必须一起调整清单与授权。

配置仅保存业务设置，不调整能力声明或恢复撤权。具体字段见 [settings.example.json](settings.example.json) 和包内实现，公共权限说明见[示例索引](../README.md)。

## 更新、撤权与卸载

```sh
sai plugins install ./新版本目录 --replace
sai plugins disable web-images
sai plugins remove web-images
```

使用 `sai plugins enable web-images` 的 `--no-http`、`--no-model`、`--no-tools`、`--no-processes`、`--no-file-write` 等对应选项撤销能力；只指定需要撤销的项，其余保持原值。普通重新启用不会恢复权限。替换安装保留设置与现有授权，新增声明须明确授权；已有会话通过 `/plugins reload` 使用新快照。

下载图片默认保存在 `~/Pictures/sai/web-images`，文件名基于内容摘要。卸载保留这些图片，升级不需要转换图片数据。 源码回退使用保留的旧发行包，与数据删除分开处理。

找不到工具时检查安装、启用状态和完整名称；权限错误检查 `sai plugins info web-images --json`。缺少模型、系统工具、网络来源或可选插件时按错误信息修正对应依赖。验证使用隔离路径、本地服务和固定样本，不代表已经实测所有平台或外部供应商。

