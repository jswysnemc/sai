# Nerd Font 图标尺寸调查

调查日期：2026-10-31。图标码点、图标后的空格和用户终端配置均未改动。

## 字体变体与空格不是同一件事

Nerd Fonts 官方区分三种变体，不是同一字体通过空格切换两种字号：

| 变体 | 图标绘制范围 | 字体前进宽度 | 空格用途 |
| --- | --- | --- | --- |
| Nerd Font Mono / NFM | 缩放到一列内 | 一列 | 正常文字间距 |
| Nerd Font / NF | 通常约 1.5 列，图形可伸到下一列 | 仍为一列 | 避免后续文字覆盖伸出的图形 |
| Nerd Font Propo / NFP | 较大图形 | 与实际图形宽度相符 | 不依赖额外空格避让，适合比例布局 |

官方 FAQ 明确指出：希望图标更大时选不带 Mono 的变体；Mono 适合严格等宽环境。补丁工具说明进一步区分图形宽度和 advance width，并明确 NF 需要图标后空格，Propo 不需要。

来源：
- https://github.com/ryanoasis/nerd-fonts/wiki/FAQ-and-Troubleshooting#which-font
- https://github.com/ryanoasis/nerd-fonts/wiki/ScriptOptions

## 空格确实可能影响同一字形的显示尺寸

### WezTerm

`allow_square_glyphs_to_overflow_width` 默认是 `WhenFollowedBySpace`：当下一单元格为空格时允许宽图形溢出本列。`Never` 严格限制列宽；`Always` 不论后面是否为空格均允许溢出。官方说明该策略适用于纵横比大于 0.9 的字形。

这属于终端绘制策略，并不代表文本切换了字体字号或 Unicode 码点。

来源：https://wezterm.org/config/lua/config/allow_square_glyphs_to_overflow_width.html

### Kitty

官方明确说明：私用区字符和部分符号后存在一个或多个空格时，若字体中字形纵横比较宽，Kitty 会利用额外单元格把图形绘制得更大。`narrow_symbols codepoints [cells]` 可以限制指定码点的绘制列数，默认一列。

因此，可通过终端设置保留图标后间距，同时限制图形尺寸；直接移除应用中的空格并不是跨终端可靠的字号控制方法。

来源：https://sw.kovidgoyal.net/kitty/conf/#opt-kitty.narrow_symbols

### Ghostty

自 1.2.0 起，`adjust-icon-height` 支持按像素或百分比调整 Nerd Font 图标的最大高度。负值降低最大高度，但实际效果取决于图形是否受高度约束，以及图标与主字体的纵横比。Powerline 等终端图形不受此选项影响。

这说明部分终端可以单独调整图标相对大小，不能笼统断言终端一律不支持。

来源：https://ghostty.org/docs/config/reference#adjust-icon-height

## 对 sai 的判断

- `src/render/guide_symbols.rs` 使用既有私用区码点，例如命令 U+F489、读取 U+F15C；布局按一列计算。
- `src/render/tool_event_line.rs` 的工具标题在图标与正文间保留空格。这符合普通 NF 的避让要求，也可能触发上述终端的大图形绘制。
- 当前信息不能确认截图所用终端、版本、图标字体及映射配置。PTY 测试只能验证列数和控制序列，不能证明真实字形的视觉尺寸。
- 不替换码点，不移除全局图标间距，不把图标统一计为两列。缩小效果应先在实际终端使用相同图标对比字体变体和对应绘制设置。

## 检索与证据

综合搜索端点 TLS 连接失败；Exa 文档搜索和 Tavily 网页读取通过连接检查。本次使用 `smart-search exa-search` 定位官方来源，再使用 `smart-search fetch` 核对正文。

```sh
smart-search exa-search 'Nerd Fonts FAQ Mono Propo glyph size icon spacing one cell two cells' --include-domains github.com nerdfonts.com --num-results 5 --include-text --format json
smart-search fetch 'https://github.com/ryanoasis/nerd-fonts/wiki/FAQ-and-Troubleshooting' --format json
smart-search fetch 'https://github.com/ryanoasis/nerd-fonts/wiki/ScriptOptions' --format json
smart-search fetch 'https://wezterm.org/config/lua/config/allow_square_glyphs_to_overflow_width.html' --format markdown
smart-search fetch 'https://sw.kovidgoyal.net/kitty/conf/#opt-kitty.narrow_symbols' --format markdown
smart-search fetch 'https://ghostty.org/docs/config/reference#adjust-icon-height' --format markdown
```
