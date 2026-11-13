# Codex 终端浅色主题与对比度规则

## Sai 本次实现与验证

- 启动时有界查询真实默认前景/背景；未知背景保持未知，不伪造白色作为成功探测结果。
- 工具悬停不绘制背景，深色背景提亮、浅色背景降低前景亮度以保持对比度。
- 正文按实际背景与 sRGB 亮度修正至 4.5:1，清除可能二次减淡文字的 dim；保留粗体、实际背景、图片和 OSC 控制数据。256 色修正检查量化后结果。
- 配置表单、常规终端帧、侧栏及后台任务独立界面都使用可读文本修正。
- 新增真实伪终端浅色回归，默认前景 `#202020`、带蓝灰色背景 `#c2d5e7`；读取正文、换行和可见 RGB 文本对比度检查通过，不仅验证纯白底。
- 工具详情统一显示竖向引导列，折行续列避开行号和引导线；读取结果不再回退到旧参数表。
- 本地命令和读取进度不再叠加旧折角或竖线；正文原始符号保持不变。真实伪终端完整回归 190 项通过，包含启动输入、折行、浅色主题、鼠标操作与图片展示。

终端只提供 ANSI16 或完全不响应颜色查询时，仍依赖用户终端调色板；本次没有把所有真实终端与所有主题都视为已经实测。

## 范围

- 日期：2026-11-14；Codex 本地基准提交：`627b579219d492b5445d6d433d3110eb53e507dc`。
- 基准调研阶段仅阅读源码，没有修改终端代码、控制桌面或运行视觉测试；Sai 实现后的验证结果见“本次实现与验证”。
- Sai 对照针对调研时读取的文件；此前已修改的 hover 实现属于工作区修改，不将它视为基准提交原始行为。

## 可直接参考的规则

### 终端背景探测

- `codex-rs/tui/src/terminal_probe.rs:251` 通过 OSC 10/11 查询默认前景与背景，探测组预算默认250ms（`:27`）。
- 探测只能在普通事件流未启动或暂停时读取终端；启动探测会把误读到的用户输入重新送给 crossterm，避免吞掉按键或粘贴（`:10`、`:295`、`:325`）。
- `codex-rs/tui/src/tui.rs:516` 执行启动探测，再在`:553`填充默认颜色缓存；失败保留None。
- `codex-rs/tui/src/terminal_palette.rs:153` 将“已尝试但未知”缓存起来，避免每帧重新查询。`default_fg()`/`default_bg()`返回Option，不用一组固定灰字白底伪装探测成功。
- `codex-rs/tui/src/color.rs:1` 用 `0.299R + 0.587G + 0.114B > 128` 判断浅色，浅色判断与正文对比度公式是两件事。

### 颜色能力与回退

- `terminal_palette.rs:14` 区分TrueColor、Ansi256、Ansi16和Unknown，而不仅是一个真彩色布尔值。
- Windows Terminal在没有FORCE_COLOR显式覆盖时补充真彩色识别（`:58`）。FORCE_COLOR不是背景主题选择，它控制颜色能力覆盖。
- 256色选择最接近期望色的固定xterm扩展色，跳过前16个终端主题自定义色（`:78`、`:231`）。只有16色或能力未知时，很多动态RGB样式退回终端默认前景。
- 不应把用户终端ANSI1–16的颜色固定解释成标准xterm RGB。Codex明确将这些颜色留给终端主题（`style.rs:174`、`render/highlight.rs:78`）。

### 弱化文字与 footer

- `codex-rs/tui/src/style.rs:193` 的 `secondary_text_style()`不用ANSI dim模拟弱化。
- 已知默认前景和背景时，先按0.6权重保留前景、0.4混入背景；再经过 `readable_color_on()` 校正对比度。
- 最终明确 `not_dim()`、`not_bold()`，避免终端继续对已经变浅的RGB文字二次减淡。
- footer中的辅助文案、分隔与提示使用这个样式，例如 `bottom_pane/footer.rs:325`、`:332`、`:338`。

### 正文对比度

- `codex-rs/tui/src/style/contrast.rs:18` 的正文最小对比度为4.5:1。
- 比较的是文字与“实际绘制在其背后的底色”，包括选中行、差异行或其它着色表面；不能总拿终端全局背景比较（`:80`；`style.rs:175`）。
- 真彩色：原色满足4.5就保留；否则比较黑、白与背景的对比度，选择更好的端点，用有界搜索做尽量少的混合，保留原有色相（`contrast.rs:95`）。
- 256色：从16–255中找满足4.5且与期望色感知距离最小的颜色（`:115`）。量化之后仍要满足对比度，不能只检查量化前RGB。
- ANSI16或颜色能力未知：返回终端默认前景，避免强制RGB与终端主题不兼容（`:125`）。
- 对比度采用sRGB线性化和相对亮度公式，非简单灰度差（`:135`）。颜色结果缓存最多512项，背景和颜色能力都是缓存键，主题变化不复用旧结果（`:21`、`:127`）。
- 这不是“所有颜色都提高亮度”。浅底通常需要把文字加深，深底通常需要把文字提亮；方向由实际对比度决定。

### 语法高亮与用户覆盖

- 自动语法主题在浅底选 `catppuccin-latte`，深底或背景未知时选 `catppuccin-mocha`（`render/highlight.rs:209`）。
- 用户配置 `[tui] theme = "主题名"` 优先于自动选择（`config/src/types.rs:1004`；`render/highlight.rs:230`）。
- 支持内置主题与 `$CODEX_HOME/themes/<name>.tmTheme`。主题不存在或文件无效时给出可操作的说明并回退默认主题（`render/highlight.rs:134`、`:199`、`:407`）。
- `/theme`可以实时预览。设置运行时主题后增加THEME_REVISION，渲染缓存按revision失效（`render/highlight.rs:268`；`history_cell/markdown_render_cache.rs:17`）。
- 通用高亮转换保留主题前景，不写主题背景，避免覆盖终端底色；忽略主题自带italic/underline，保留bold（`render/highlight.rs:581`）。
- `ansi`/`base16`等主题通过alpha标记区分ANSI索引、终端默认色与RGB，不应破坏这些含义（`render/highlight.rs:563`）。
- 通用代码渲染有意保留有效用户主题颜色；差异渲染单独按真实增删底色计算对比度。不能宣称Codex对所有语法颜色无条件强制4.5（`render/highlight.rs:21`、`:780`）。

## 需要明确的 Codex 限制

Codex并未全面删除dim。`codex-rs/tui/src/history_cell/messages.rs:448` 的 `ReasoningSummaryCell` 仍给展开思考正文叠加dim+italic，并让圆点dim（`:454`、`:474`）。其它装饰和未知背景回退也可能使用dim。

因此，修复Sai截图中的思考文字不能以“Codex思考正文已完全不用dim”为依据。可以参考Codex的辅助文字和对比度工具，但思考正文需要独立落实可读性规则并验证。

## Sai 对照与本次截图的直接风险

| 文件 | 调研时的行为 | 风险/修复依据 |
| --- | --- | --- |
| `src/render/terminal_palette.rs:20` | 输入循环前通过terminal-colorsaurus探测一次，200ms | 已具备基本探测，不应把问题归因于完全没有浅色识别；需要保留未知状态与输入读取边界 |
| `src/render/terminal_palette.rs:58` | 探测未知时全局回退前景128灰、背景255白 | 与Codex的Option语义不同；可能让未知背景被错误判成浅色，不能声称这是Codex统一缺省调色板 |
| `src/render/panel_palette.rs:30` | 根据亮度选择浅/深固定前景、muted等，再融合边框/底色 | 能选择浅色但没有正文4.5对比度校验；面板配色存在不等于footer或思考正文已使用 |
| `src/cli/repl_runtime/fullscreen/hover_color.rs:3` | 所有显式前景无条件向白色混合，默认hover前景纯白；SGR22仅清除bold/dim | 浅背景下hover会降低对比度，默认色可能消失；应按背景提高对比度而非固定向白色移动 |
| `src/cli/repl_runtime/fullscreen/hover_color.rs:62` | 前16色按固定ANSI16数组转换成RGB | 可能覆盖用户终端浅色ANSI主题；需区分终端定义色和固定扩展色 |

“取消hover背景”与“可读性增强”可以同时保留：不新增底色，仅将当前前景向能提高实际对比度的方向调整，必要时清除dim。无需把工具区全部改成一种纯黑或纯白颜色，也不应洗掉语法色。

## 建议验证范围

- 白底、米白底、中灰底、深底；正文、思考正文、footer和提示文字分别测试。
- 已知前景背景、探测失败、ANSI16、ANSI256、真彩色分别验证，特别检查量化后的对比度。
- 原文本有SGR2、SGR22、SGR0、多段语法颜色时，hover不能引入背景、遗留dim或覆盖后续ANSI样式。
- 浅色hover不应向白色移动；深色hover不应变成灰黑；原本已满足要求的语法色尽量保留。
- 用户显式语法主题与自动主题区分；主题变化时旧渲染缓存应失效。
- Codex已有 `style/contrast_tests.rs:100` 的白/米白/中灰/深色矩阵，`:130` 的实际着色背景验证，以及`:155`的缓存边界测试可作参考。本调研未执行这些测试。

## 核心源码路径

Codex路径以 `/home/snemc/workspace/codex/` 为根：

- `codex-rs/tui/src/terminal_probe.rs:27`、`:251`、`:295`、`:832`
- `codex-rs/tui/src/terminal_palette.rs:14`、`:58`、`:105`、`:153`
- `codex-rs/tui/src/color.rs:1`
- `codex-rs/tui/src/style.rs:175`、`:193`
- `codex-rs/tui/src/style/contrast.rs:18`、`:80`、`:135`
- `codex-rs/tui/src/render/highlight.rs:106`、`:209`、`:230`、`:268`、`:563`、`:581`
- `codex-rs/config/src/types.rs:1004`
- `codex-rs/tui/src/bottom_pane/footer.rs:325`
- `codex-rs/tui/src/history_cell/messages.rs:448`
- `codex-rs/tui/src/style/contrast_tests.rs:100`

定位使用Fast Context与已知范围精确检索；未使用二手文章作为行为证据。`tui.theme`字段也与已获取的官方Configuration Reference一致。
