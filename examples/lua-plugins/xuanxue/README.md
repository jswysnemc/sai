# 抽取与骰子示例

提供周易卦象、塔罗牌、签文抽取与骰子计算。使用包内数据和公共随机接口，无网络、模型或系统能力，不依赖其他插件。

## 安装、授权与运行

以下命令从仓库根目录执行；独立分发时将源码路径替换为解压后的包目录。

```sh
sai plugins check examples/lua-plugins/xuanxue
sai plugins pack examples/lua-plugins/xuanxue --output ./xuanxue-1.0.0.tar.gz
sai plugins install examples/lua-plugins/xuanxue
sai plugins configure xuanxue examples/lua-plugins/xuanxue/settings.example.json
sai plugins enable xuanxue
sai --plan plugins call xuanxue roll_dice '{"count":2,"sides":6,"modifier":1}'
```

安装后默认禁用且没有授权。以上授权覆盖示例所需能力，模型工具名为 `lua__xuanxue__roll_dice`；其他工具同样使用该包前缀。安装不修改 Agent 白名单。支持 sai 对应公共宿主可用的平台；网络查询需要访问所选服务，固定样本测试不代表外部服务实时可用。

## 参数、配置与依赖

工具包括 `draw_zhouyi_hexagram`、`draw_tarot_card`、`draw_fortune_lot` 和 `roll_dice`。前三个工具接受 `{}`；骰子默认一颗六面骰，数量最多 100，面数最多 1000，`modifier` 调整总和。随机结果每次可变；测试核对数据集合与数值边界。

本包没有业务设置，设置样本为 `{}`。 旧主配置开关、原内置短名称均不再提供本包；通过插件管理入口保存独立配置和授权。

## 更新、撤权与卸载

```sh
sai plugins install ./新版本目录 --replace
sai plugins disable xuanxue
sai plugins remove xuanxue
```

替换安装保留设置与现有授权，新增声明不自动获得授权；普通重新启用不会恢复已撤销权限。本包没有外部权限可撤销，禁用后工具不再注册。 已有会话使用 `/plugins reload` 更新快照。

本包不保存业务数据。卸载删除已安装源码，保留禁用配置；回退旧源码不需要转换业务数据。找不到工具时核对安装、启用状态及完整名称；权限错误通过 `sai plugins info xuanxue --json` 检查，配置错误须修正相应字段。管理流程和归档边界见[示例索引](../README.md)。

