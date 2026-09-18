# 自定义 TUI 底栏

普通外部 Lua 插件，通过 `tui_status` 纯回调配置底栏显示内容。默认效果：

```text
yolo · model-name · 25.0%/128k · cache 90%                         sai
```

需要包含 `tui_status` 接口的 Sai 构建。旧版宿主会拒绝清单中的新能力字段。

## 安装与配置

在仓库根目录执行；本地开发构建可把 `sai` 换成 `target/debug/sai`：

```sh
sai plugins check examples/lua-plugins/tui-status
sai plugins install examples/lua-plugins/tui-status
sai plugins configure tui-status examples/lua-plugins/tui-status/settings.example.json
sai plugins enable tui-status --allow-tui-status
```

安装只复制清单与 Lua 源码。将 `settings.example.json` 复制到自己的配置文件中修改，再用 `plugins configure` 保存。已打开的 TUI 在下一次进入输入循环时读取设置；空闲时输入 `/plugins reload` 即可生效。

| 配置 | 默认值 | 含义 |
| --- | --- | --- |
| `left` | `["mode", "model", "context", "cache"]` | 左侧字段及顺序；空数组隐藏这一侧 |
| `right` | `["directory"]` | 右侧字段及顺序 |
| `separator` | `" · "` | 字段分隔符，最多 24 字节，不接受控制字符 |
| `directory_style` | `"name"` | `name` 只显示目录名，`path` 显示宿主已有的压缩路径 |
| `label` | `"Sai"` | `label` 字段的自定义文字，最多 80 字节 |
| `compact_below` | `60` | 小于该列数时隐藏 `thinking`、`cache`；设为 0 可关闭 |

支持字段：`mode`、`model`、`thinking`、`context`、`cache`、`directory`、`label`。同一侧不能重复字段。缓存命中率没有读数时自动省略；模式、上下文和缓存读数随宿主状态更新。

只看模型、思考等级与目录：

```json
{
  "left": ["model", "thinking"],
  "right": ["directory"],
  "separator": " / ",
  "directory_style": "name"
}
```

停止快捷键和活动提示由宿主保留。着色、宽度计算和裁剪沿用 TUI；插件只返回左右纯文本。窄窗口优先保留左侧，超长内容会裁剪。

## 运行边界与验证

插件不注册模型工具，不需要网络、模型、文件或存储授权。展示在独立后台运行时计算，重复快照合并；回调失败或超时会显示诊断并回退默认底栏，不阻塞按键。多个底栏插件按 ID 排序，使用第一个非空结果，最多载入八个。

```sh
cargo test --locked tui_status
```

测试通过普通安装、设置和授权入口运行本包，验证底栏实际渲染、字段排序、模式与用量更新、窄终端、配置拒绝和撤权。运行时测试另验证宿主 I/O 隔离、控制字符拒绝、输出上限及失控回调终止。

Linux 可使用真实伪终端验收安装、输入草稿、缩放、重新加载和禁用：

```sh
cargo build --locked
uv run scripts/plugin-smoke/verify_tui_status.py --output /tmp/sai-tui-status-smoke.json
```

脚本使用隔离 XDG 目录，不修改现有插件配置，也不发送模型请求。

恢复默认底栏：

```sh
sai plugins disable tui-status
```

只撤销底栏权限：`sai plugins enable tui-status --no-tui-status`。源码接口见[底栏插件契约](../../../design/lua-plugins/tui-status-api.md)。
