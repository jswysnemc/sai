# 本地知识库示例

此包提供六个 `lua__knowledge-base__…` 工具，以及导入、查询、编辑、删除、索引和后台嵌入命令。没有其他插件依赖；文件、SQLite、锁与任务执行通过公共宿主完成。CLI 和 TUI 的知识库管理入口只有在安装并启用后才可使用，缺失时可选列表为空；Web 对话可按 Agent 白名单调用已安装工具。

## 安装与本地查询

```sh
sai plugins check ./examples/lua-plugins/knowledge-base
sai plugins pack ./examples/lua-plugins/knowledge-base --output ./knowledge-base.tar.gz
sai plugins install ./examples/lua-plugins/knowledge-base
sai plugins configure knowledge-base ./examples/lua-plugins/knowledge-base/settings.example.json
sai plugins enable knowledge-base --allow-plugin-storage --allow-read-path kb --allow-write-path kb --allow-remove-path kb
sai --yolo plugins run knowledge-base add-text '{"name":"notes/example.md","content":"本地知识资料","format":"json"}'
sai --plan plugins call knowledge-base search_knowledge_base '{"query":"知识资料"}'
sai --yolo kb list
```

默认库位于本次工作目录的 `kb/`，包含 `files/`、`kb_meta.db` 和 `semantic_index.db`。只读工具不初始化目录；管理命令可初始化或恢复索引，因此要求写入模式。默认单文件上限 1 MiB，可配置至 4 MiB；格式与索引限制见[知识库设计](../../../design/lua-plugins/knowledge-base.md)。

`add-text` 保存原始 UTF-8 正文；聊天上传工具另行添加 Markdown 标题和来源信息。TUI 与 CLI 共用 `add` 命令，支持文件或目录，来源读取必须已经获得授权。

从文件导入需先把来源放在 `.sai/kb-import` 并授权该目录：

```sh
sai plugins enable knowledge-base --allow-read-path kb --allow-read-path .sai/kb-import
sai --yolo kb add .sai/kb-import/example.md
```

同类路径参数会替换该项授权集合，授权时列出仍需使用的全部目录。自定义库或输入目录必须同时修改清单与授权，`data_dir` 设置不会自动扩权。旧 `input_paths` 主配置投影和临时输入授权已经移除。

## 独立嵌入设置

默认关闭嵌入。启用时设置 `embedding_enabled`、`embedding_provider_id`、`embedding_model` 和 `provider`；`provider` 包含完整 `endpoint`、可选 `api_key`、可选 `api_key_env`。这些设置不读取宿主模型供应商、主密钥或私密配置。

默认端点为 `https://api.openai.com/v1/embeddings`，环境名为 `SAI_KB_EMBEDDING_API_KEY`。显式密钥优先；环境值只在调用时通过公共环境接口读取。需要另外授权 HTTP 来源、精确只读 POST 端点、环境变量及调度能力：

```sh
sai plugins enable knowledge-base --allow-http https://api.openai.com --allow-http-read-only-post https://api.openai.com/v1/embeddings --allow-env SAI_KB_EMBEDDING_API_KEY --allow-schedule
sai --yolo kb embed reindex
```

自定义端点或变量先修改清单，再替换安装并明确授权。保留原 `embedding_provider_id` 和模型名称可以继续识别旧语义行。网络失败沿用关键词回退，后台任务不自动借用其他供应商。

## 更新、撤权与卸载

```sh
sai plugins install ./new-knowledge-base --replace
sai plugins enable knowledge-base --no-http --no-env --no-schedule
sai plugins disable knowledge-base
sai plugins remove knowledge-base
```

源码更新保留设置与授权，卸载默认保留知识文件、数据库、私有协调记录和任务历史。旧库通过明确设置原数据目录并授权接续，不自动复制或删除。未完成的 `pending-write.json` 由后续获授权的写入入口恢复；恢复遇到外部修改会拒绝覆盖。

禁用或卸载后不再执行嵌入；现有任务可通过 `plugins jobs knowledge-base list/cancel` 管理。源码回退不等于数据库回退，恢复旧数据应使用事先备份并停止并发写入。
