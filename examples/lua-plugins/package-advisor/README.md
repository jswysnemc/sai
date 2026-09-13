# AUR 软件包审查与安装

审查 AUR 元数据及构建文件，保存会话审查记录，并在后续用户确认后执行安装。实际安装仅适用于具有 Arch 工具链的 Linux 环境。

## 安装、配置与运行

从仓库根目录执行；独立分发后将路径替换为自己的包目录。

```sh
sai plugins check examples/lua-plugins/package-advisor
sai plugins pack examples/lua-plugins/package-advisor --output ./package-advisor-1.0.0.tar.gz
sai plugins install examples/lua-plugins/package-advisor
sai plugins configure package-advisor examples/lua-plugins/package-advisor/settings.example.json
sai plugins enable package-advisor --allow-http https://aur.archlinux.org --allow-session-storage --allow-workspace --allow-env PATH --allow-env HOME --allow-env LANG --allow-process paru_version --allow-process paru_fetch --allow-process yay_version --allow-process yay_fetch
sai --plan plugins call package-advisor review_aur_package '{"package":"example-package"}'
```

安装默认禁用且没有授权。模型使用的工具名称为 `lua__package-advisor__review_aur_package`，其他入口也使用同样的包前缀。安装不会修改 Agent 白名单。配置样本不包含真实密钥或用户数据；旧主配置选项不再注入本包。

## 参数、权限与依赖

以上授权用于审查：优先尝试 paru、yay，再使用下载解压的构建文件。网络、进程模板、环境、隔离工作目录及会话存储分别授权。缺少依赖或文件不完整时返回诊断，不建立可安装的审查状态。

`install_aur_package` 需要 `user_confirmed=true`、同一会话已有允许安装的审查，以及不同于审查轮次的可信操作标识。审查记录只能原子消费一次。安装步骤另需允许写入模式，以及清单中实际使用的 `paru_install`、`yay_install`、`makepkg`、`pacman_install` 模板。应在交互会话中展示审查结果，收到用户后续确认后执行；单独的命令行演示只展示审查，不构成安装确认。没有隐藏的原生命令回退。

配置仅保存业务设置，不调整能力声明或恢复撤权。具体字段见 [settings.example.json](settings.example.json) 和包内实现，公共权限说明见[示例索引](../README.md)。

## 更新、撤权与卸载

```sh
sai plugins install ./新版本目录 --replace
sai plugins disable package-advisor
sai plugins remove package-advisor
```

使用 `sai plugins enable package-advisor` 的 `--no-http`、`--no-model`、`--no-tools`、`--no-processes`、`--no-file-write` 等对应选项撤销能力；只指定需要撤销的项，其余保持原值。普通重新启用不会恢复权限。替换安装保留设置与现有授权，新增声明须明确授权；已有会话通过 `/plugins reload` 使用新快照。

审查记录保存在本包所属会话存储，构建文件位于公共工作目录服务提供的隔离缓存。卸载保留历史记录和缓存；它不会卸载已经安装到系统的软件。 源码回退使用保留的旧发行包，与数据删除分开处理。

找不到工具时检查安装、启用状态和完整名称；权限错误检查 `sai plugins info package-advisor --json`。缺少模型、系统工具、网络来源或可选插件时按错误信息修正对应依赖。验证使用隔离路径、本地服务和固定样本，不代表已经实测所有平台或外部供应商。

