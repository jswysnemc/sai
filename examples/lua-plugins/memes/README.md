# 图库示例

此包提供六个 `lua__memes__…` 工具，支持查询、显示、新增、编辑、删除和最近发送记录。自动发送策略默认关闭，没有其他插件依赖；终端显示通过公共图片接口完成，自动元数据使用独立视觉配置。

## 安装与目录

```sh
sai plugins check ./examples/lua-plugins/memes
sai plugins pack ./examples/lua-plugins/memes --output ./memes.tar.gz
sai plugins install ./examples/lua-plugins/memes
sai plugins configure memes ./examples/lua-plugins/memes/settings.example.json
sai plugins enable memes --allow-read-path .sai/meme-bases --allow-read-path .sai/memes --allow-read-path .sai/state/memes
sai --plan plugins call memes search_meme '{"query":"思考"}'
```

`builtin_dirs` 是显式安装的只读基础图库，默认 `.sai/meme-bases`；可写覆盖库为 `.sai/memes`，发送记录为 `.sai/state/memes`。这些路径均相对于本次调用的工作目录。每个库使用 `<目录>/<库名>/index.json` 和 `images/`，默认库名为 `sai`。

可选的 [assets/sai](assets/sai/) 保留原图库图片与索引。需要示例素材时，在所选工作区执行：

```sh
mkdir -p .sai/meme-bases
cp -R ./examples/lua-plugins/memes/assets/sai .sai/meme-bases/sai
```

插件归档只收录清单和 Lua 源码，图片作为独立数据分发；宿主安装包不再携带图库。自定义路径必须同时修改清单、设置和授权。插件不读取旧主配置或 `SAI_MEMES_DIR`，也不会搜索仓库源码目录。

## 写入与显示

把待导入图片复制到 `.sai/meme-import` 后授权：

```sh
sai plugins enable memes --allow-read-path .sai/meme-bases --allow-read-path .sai/memes --allow-read-path .sai/state/memes --allow-read-path .sai/meme-import --allow-write-path .sai/memes --allow-remove-path .sai/memes --allow-trash-path .sai/memes
sai --yolo plugins call memes add_meme '{"image":".sai/meme-import/example.png","name_zh":"思考","description":"用于讨论方案","usage":"讨论方案时使用","tags":["思考"]}'
sai --yolo plugins call memes update_meme '{"id":"sha256:图片摘要","usage":"讨论方案"}'
sai --yolo plugins call memes delete_meme '{"id":"sha256:图片摘要","hard_delete":true}'
```

同类路径参数会替换该项授权集合，应列出需要保留的全部目录。新增需要删除权限，以清理并发冲突产生的临时图片。默认删除进入系统回收站，`hard_delete:true` 才永久删除；只读基础图库仅记录禁用，不删除原图片。

手工新增必须提供 `name_zh`、`description` 和 `usage`；完全省略元数据时需要 `--allow-vision` 和宿主独立视觉配置。展示另需 `--allow-image-display`，例如 `sai --plan plugins call memes show_meme '{"id":"sha256:图片摘要"}'`。GIF 当前使用静态预览，显示依赖可用的终端与平台后端。

自动发送还需设置 `auto_send_enabled:true`，并分别授予 `--allow-reply-policy --allow-model --allow-image-display --allow-write-path .sai/memes --allow-write-path .sai/state/memes`。只读模式不安排自动发送；撤销策略授权后，手工工具仍可独立使用。

## 更新与数据保留

```sh
sai plugins install ./new-memes --replace
sai plugins enable memes --no-reply-policy --no-model --no-vision --no-image-display
sai plugins disable memes
sai plugins remove memes
```

更新保留设置和已有授权，新能力需要明确授权。卸载保留基础图库、用户索引、图片和发送记录。接续旧数据时，将 `builtin_dirs`、`user_dir`、`state_dir` 指向原目录并同步声明、授权，不自动复制或修改旧数据。

损坏索引、未授权目录和越界图片路径会报错。删除中断保留待处理记录，通过相同工具显式重试；源码回退不回退数据，也不保证旧程序理解新的删除恢复记录。格式、并发与取消边界见[图库设计](../../../design/lua-plugins/memes.md)。
