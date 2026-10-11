# 设置分区契约

分区元数据定义在 `settings-sections.ts`，解析、过滤与兼容映射位于
`settings-registry.ts`。页面通过 `SettingsSectionBody` 挂载领域组件。

## 配置与保存

| appConfig | 数据来源 | 全局保存 | 加载处理 |
| --- | --- | --- | --- |
| required | 全局 AppConfig | 常驻，仅在有有效草稿时可用 | 等待配置加载，失败显示恢复入口 |
| optional | 独立接口与部分 AppConfig 字段 | 全局草稿有修改时显示 | 配置不可用时按分区降级 |
| none | 独立接口或浏览器偏好 | 全局草稿有修改时仍显示 | 由分区管理 |

- `useConfigDocument` 管理服务端快照、配置草稿与保存请求。
  请求期间的新编辑保留为未保存草稿；同一文档不并行发起保存。
- `useSettingsConfig` 组合供应商、网关和完整 JSON 编辑。非法 JSON 文本跨分区保留，
  禁止保存和保存快捷键，直到修正或放弃；结构化编辑会以新的配置重建 JSON 文本。
- 底部保存栏提供保存、放弃及 `Ctrl/Cmd+S`；快捷键优先保存当前独立文档。
  路由守卫覆盖链接及浏览器前进、后退；全局草稿仅在离开设置时确认，
  MCP 和 Skill 文档在即将卸载时确认，Skill 同路径切换对象也受保护。
- `configKeys` 定义导航未保存标记所比较的配置路径。非法 JSON 单独标记高级分区。
- 即时操作和独立文档保存不经过全局 AppConfig，保存说明来自分区元数据。

## 分区清单

| id | 分组 | appConfig | 数据来源与保存方式 |
| --- | --- | --- | --- |
| providers | basics | required | api.config；供应商、凭据、模型及探测 |
| image-models | basics | required | api.config；生图模型端点 |
| appearance | basics | none | 浏览器主题、语言、聊天阅读列与 Markdown 偏好，即时生效 |
| runtime | basics | required | api.config；内核、会话、权限、终端、工具、上下文与显示 |
| prompts | basics | required | api.config；内部提示词模板，恢复前显示差异 |
| git | basics | required | api.config；Git 与源代码管理行为 |
| ssh | basics | none | SSH 主机接口，独立保存 |
| agents | basics | required | api.config 与 Agent 接口 |
| cli-tools | agentCapabilities | required | api.config；CLI 助手工具 |
| web-search | agentCapabilities | required | api.config；网页搜索与凭据 |
| computer-use | agentCapabilities | required | api.config；桌面操作开关、驱动路径与超时；驱动安装经独立接口确认后执行 |
| jev | agentCapabilities | required | api.config；Jev 接入与决策 |
| skills | agentCapabilities | optional | 技能文档即时保存；技能行为通过底部保存栏保存 |
| mcp | agentCapabilities | none | MCP 独立配置文档，本节保存 |
| hooks | agentCapabilities | required | api.config；生命周期 Hooks |
| gateways | agentCapabilities | required | api.config；消息网关配置与运行操作 |
| memory | agentCapabilities | optional | 记忆内容操作即时生效；配置字段通过底部保存栏保存 |
| session-data | dataAndStats | none | 会话数据接口；查询与清理 |
| usage | dataAndStats | none | 用量统计查询；日志清空为确认后即时操作 |
| advanced | dataAndStats | required | 全局 AppConfig JSON 草稿 |

运行时的网页终端字体、字号和回滚行数仅保存到浏览器，即时更新已打开终端；
Shell 仍通过底部保存栏保存。SSH 设置复用终端创建与主机密钥确认，可收起或明确结束会话。
主题悬停仅临时预览，点击才保存；聊天阅读列的字体、字号和宽度只改对话正文；Markdown 配置与效果预览在宽屏下并列。
JSON 编辑器统一提供复制、格式化与全屏编辑，切换窗口保留光标和草稿。
网页搜索密钥按原始数组槽位替换或清除，隐藏密钥不因其他条目删除而移动。

分组名称依次为“基础”“智能体能力”“数据与统计”。

## 子页与兼容地址

- `/settings` 与未知分区重定向至 `/settings/providers/connection`。
- 有子页的分区使用 `/settings/:sectionId/:subview`，缺省或未知子页归一到首个子页。
- 无子页分区使用 `/settings/:sectionId`。
- 旧分区 `plugins` 映射到 `cli-tools`，`jev-models` 映射到 `jev`。
- 供应商子页：`connection`、`models`、`behavior`、`advanced`。
- 运行时子页：`execution`、`environment`、`tools`。
  旧 `engine` 映射到 `execution`，`permissions` 和 `terminal` 映射到 `environment`，
  `context` 映射到 `tools`。
- Jev 不再注册二级子页。
- 用量子页：`overview`、`breakdown`、`logs`。
  旧 `providers`、`models`、`sessions` 映射到 `breakdown`，并以 `view` 保留原维度。
- 地址归一化保留查询参数；对象选择使用 `item`，Agent 详情页签和用量维度使用 `view`。

## 字段与布局

- 分区标题在内容区页头显示一次，由 `SettingsPageHeader` 渲染 22px 标题与 13px 说明。
  顶栏只保留桌面拖拽条，不再重复分区名。
- `SettingsPanel` 提供 13px 分区标签，并把内容包进 `SettingsGroup` 卡片。
- `SettingsField` 默认左标题右控件；宽控件或 `span="full"` 改为上下排布。
- `SwitchField` 标题在左、开关固定在右，整行可点击。
- `FieldGrid` 在卡片内展开为单列细分隔行，列数属性仅保留给调用方。
- `configKey` 只作为字段提示与检索数据；说明文字可换行。
- 主内容统一限宽约 1120px，对象列表和数据页不再单独放宽。
- `search/entries-*.ts` 按领域维护索引。`anchor` 必须匹配字段标识，
  `focus` 查询参数触发滚动与短暂高亮；条件字段未显示时提供启用或选择提示。
- 二十个分区均组合共享组件。移动端保留搜索输入，分类横向滚动，搜索结果纵向展示。

## 技能边界

技能扫描、文档创建与编辑属于 Skills 操作区；渐进式加载、命令执行等行为
配置仍在 Skills 分区编辑，通过底部保存栏保存。运行时不重复编辑这些字段。
