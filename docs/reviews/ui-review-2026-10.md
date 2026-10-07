# Sai UI 整改建议

> 评审日期：2026-10-07
> 评审范围：`pics/web.png`、`pics/web1.png`、`pics/git-model.png`、`pics/multi-subagents.png`、`pics/sai-config.png`，以及 `web/src` 前端源码（tokens 体系、chat / source-control 等 feature 的 CSS）
> 工作分支：`docs/ui-review`

---

## 评审结论速览

Sai 的 Web 工作台在**设计系统基建**上已经是开源项目里少见的高水准——token 分层清晰、9 套主题、容器查询、`prefers-reduced-motion` 都已落地。当前的短板不在基建，而在**信息层级与扫读效率**：整条时间线几乎是同一明度的灰绿文字，找不到视觉锚点；Git 视图、工具调用流、空态都有较大的优化空间。

下面按「保留 → 改进」组织，按 P0 / P1 / P2 给出优先级。

---

## 一、值得保留的设计决策（不建议推翻）

1. **Token 体系完整且纪律严明**
   `color-themes / typography / spacing / radius / motion / elevation` 分文件管理，9 套主题 + system，派生色统一走 `color-mix`，注释里写明"新增样式一律引用 token"。这是很多开源项目做不到的，务必保留。

2. **排版考虑了跨平台 CJK 垂直居中**
   `--leading-control: 1.6` 的注释解释了 DirectWrite/FreeType 的 ascent 差异，是真实踩过坑的写法。

3. **容器查询已落地**
   `chat-page` 和 diff 卡片用 `@container` 做宽度收敛，比媒体查询更适合多栏工作台。

4. **动效克制**
   时长阶梯 80–320ms，`prefers-reduced-motion` 直接禁用进场动画，没有为了炫技牺牲性能。

5. **密度定位清晰**
   基准字号 13px，面向"工作台"而非"阅读页"，信息密度优先。这个定位本身没问题，下面的建议都在"保持密度"前提下做。

---

## 二、视觉层级：灰海问题（P0）

### 现象

`web.png` / `multi-subagents.png` 里，整条时间线几乎都是 12–13px 的灰绿色文字，标题、正文、工具调用、元信息的明度差太小，视觉焦点只能靠左侧细线，扫读时找不到"这一轮在干嘛"。

### 建议（不增加字号、不降低密度）

| 层级 | 现状 | 改法 |
|---|---|---|
| Turn 标题（用户消息首行 / AI 回复首行） | 与正文同字重 | 字重升到 `--weight-semibold`（600），色用 `--ink` 而非 `--ink-soft` |
| 工具调用行（`Read file`、`Bash ...`） | 与正文同色 | 图标 + `--font-code` + `--ink-soft`，但要给状态色块（成功 `--signal` / 失败 `--danger` / 运行中 `--warning`） |
| 元信息（耗时、token、时间戳） | 与正文同字号 | 降到 `--text-xs`（11px）+ `--tracking-wide`，与正文拉开 |
| 折叠块标题（"思考过程"、"读取了 8 个文件"） | 细灰字 | 左侧 2px 同色竖条 + 字重 500，折叠态就承担导航作用 |

**关键原则**：保持 13px 基准不变，靠**字重 + 语义色 + 等宽字体**分层，而不是靠放大字号。

---

## 三、三栏工作台的宽度与呼吸感（P1）

### 现象

`web.png` 里中间对话列被左（会话列表）右（文件/Git/浏览器）挤压，在 1440px 屏上对话列大约只剩 500–600px，代码块横向滚动频繁；且三栏背景色都是 `--paper`，栏间只有 1px `--line`，长会话时边界感弱。

### 建议

1. **给对话列设最小宽度**
   `--chat-column-width` 目前是 `min(100%, var(--chat-column-width))`，建议在 `tokens/workbench.css` 里加 `--chat-column-min: 34rem`（约 544px），左右栏可压缩时优先压左右。

2. **栏间区分改用"明度差"而非"描边"**
   左右侧栏背景用 `--paper-deep`，中间对话列保持 `--paper`，去掉 1px 竖线。比加粗描边更安静，也更现代。

3. **右栏头部统一**
   目前右栏顶部的 tab（文件 / 浏览器 / Git）与 Monaco / diff 头部样式不统一，建议统一成"标题 + 图标按钮组"的 `--text-sm` 工具条，高度 2rem。

---

## 四、Git 审阅视图（P0/P1）

对应截图：`web1.png` / `git-model.png`

### 现象

文件列表 + diff 同屏时，左侧文件名等宽字体 11px 偏弱，右侧 diff 的 `+`/`-` 统计与代码颜色对比度不足；`git-model.png` 里大量 untracked 文件（`pics/*.png`）平铺，没有分组。

### 建议

1. **变更列表加分组**（P1）
   按 Staged / Unstaged / Untracked 折叠分组（`repository-change-group.css` 已有雏形，确认默认展开态），untracked 超过 10 个时默认折叠成 "… N 个未跟踪文件"。

2. **diff 统计色块化**（P0）
   现在 `+12 -3` 是纯文字，建议改为两个 2px 高的色条（绿/红）+ 等宽数字，扫读时不用看符号。

3. **文件状态字母（M/A/D）**（P0）
   从 `--ink-soft` 改为对应语义色（M `--blue` / A `--signal` / D `--danger`），并加 2px 左边框，比字母本身更快识别。

4. **diff 卡片头部 sticky 背景穿透**（P0）
   `file-diff-card.css` 里 `.git-file-card-head` 是 `background: transparent`，滚动时下方代码会透上来。改成 `background: var(--paper-raised)` + 底部 1px `--line`。

---

## 五、子代理 / 工具调用的"流"感（P1）

对应截图：`multi-subagents.png`

### 现象

`执行了 N 次操作` 这类折叠块，折叠态信息量低，展开后又是均等灰字，不知道哪一步是关键（比如哪步写了文件、哪步只是列目录）。

### 建议

1. **折叠态摘要升级**
   不要只显示次数，显示"主动词 + 关键目标"，例如：
   - `执行了 8 次操作` → `写入 3 个文件 · 读取 5 次`
   - 实现方式：在 `tool-renderers` 里按 tool 类型聚合，而不是按次数。

2. **展开态按工具类型着色左边框**
   Read `--blue` / Write `--signal` / Bash `--warning` / 出错 `--danger`，2px 即可，不占用横向空间。

3. **正在运行的工具调用加呼吸感**
   现在 spinner 是通用转圈，建议运行中的工具行背景用 `--surface-soft` + 左侧 2px `--warning` 呼吸（`@keyframes` 透明度 1 ↔ 0.5，1.2s），让用户知道"还在跑"。

---

## 六、空态与引导（P2）

### 现象

`chat-empty-state` 和 `source-control-empty-state.css` 存在，但截图里 New session 的空态只有一行提示，没有利用空间。

### 建议

1. **空对话时给出"3 个常用起手式"卡片**
   例如：
   - 阅读这个项目并生成架构图
   - 运行测试并修复失败
   - 把最近 3 个 commit 整理成 changelog

   点击直接填入 composer。比"请输入"更能留住第一次使用的用户。

2. **Git 空态**
   不只是"没有变更"，给出"初始化仓库 / 克隆 / 打开文件夹"三个入口，图标 + 一行描述。

---

## 七、配置 TUI 与 Web 端的体验一致性（P2）

### 现象

`config.png`（TUI）是白底黑框 + 键盘导航，Web 端设置页是另一套视觉，两者几乎无关联。

### 建议（不追求像素一致，追求"语义一致"）

1. Web 设置页左侧导航沿用 TUI 的分组顺序（激活配置 / 供应商和模型 / 插件配置 / …），降低双端切换成本。

2. TUI 里的 `[j/k]移动 [Enter]选择 [q]返回` 提示条，在 Web 端设置页底部可以加一条等宽的快捷键提示（`Ctrl+,` 打开设置等），强化"键盘友好"这个卖点。

---

## 八、代码层面的具体整改（可直接改）

从源码里看到的具体问题，按文件列出：

| 文件 | 问题 | 改法 | 优先级 |
|---|---|---|---|
| `web/src/features/source-control/changes/change-file-list.css` | 多处 `padding:0 var(--space-2xs)` 缺少 `padding: 0` 后的空格，风格不统一 | 统一 `padding: 0 var(--space-2xs)` | P2 |
| `web/src/features/source-control/diff/file-diff-card.css` | `.git-file-card-head` 透明背景导致 sticky 穿透 | `background: var(--paper-raised)` | P0 |
| `web/src/features/chat/chat-page.css` | `.jump-to-bottom` 用 `bottom: calc(100% + 0.35rem)` 绝对定位在滚动容器里，长列表时可能遮挡 | 改为 `position: sticky; bottom: 0.75rem` 或挂在 `composer-dock` 上方 | P1 |
| 全局（badge 类） | `--text-2xs`（10px）在部分徽章上使用，Windows 低分屏会糊 | 徽章最小字号锁定 `--text-xs`（11px），10px 仅用于图标角标 | P1 |

---

## 九、优先级路线图

按投入产出比排序：

### P0（1 天内可完成）
- [ ] diff 卡片 sticky 头背景（`file-diff-card.css`）
- [ ] 文件状态字母（M/A/D）语义着色
- [ ] diff 统计色块化
- [ ] Turn 标题字重分层（600 + `--ink`）

### P1（2–3 天）
- [ ] Git 变更分组（Staged / Unstaged / Untracked）+ Untracked 默认折叠
- [ ] 工具调用折叠摘要升级（"写入 N 个文件 · 读取 M 次"）
- [ ] 三栏明度分层（左右 `--paper-deep`，中间 `--paper`，去描边）
- [ ] `--chat-column-min: 34rem`
- [ ] `.jump-to-bottom` 改 sticky
- [ ] 徽章最小字号 11px

### P2（1 周）
- [ ] 空态引导卡片（Chat 起手式 / Git 初始化入口）
- [ ] 运行中工具呼吸动效
- [ ] Web/TUI 设置语义对齐
- [ ] `change-file-list.css` 代码风格统一

---

## 十、落地约束

执行本评审里的整改时，请遵守项目已有的硬性约束：

- **新增样式一律引用 token，不要写死 px**（`tokens.css` 顶部注释明确要求）
- **保持密度**：基准字号 13px 不动，层级靠字重 / 语义色 / 等宽字体区分
- **遵守 `prefers-reduced-motion`**：任何新增动效必须在该媒体查询下退化为即时呈现
- **保留 `color-mix` 派生**：不要为某个主题单独写死颜色
- **多主题验证**：任何视觉改动至少在 默认亮色 / graphite / dusk 三套主题下目测一遍

---

## 附：评审素材索引

- 截图：`pics/web.png`（三栏工作台）、`pics/web1.png`（Git 审阅）、`pics/git-model.png`（变更列表）、`pics/multi-subagents.png`（子代理时间线）、`pics/sai-config.png`（配置 TUI）
- 关键源码：
  - `web/src/shared/styles/tokens.css` 及 `tokens/` 目录
  - `web/src/features/chat/chat-page.css`
  - `web/src/features/source-control/diff/file-diff-card.css`
  - `web/src/features/source-control/changes/change-file-list.css`

---

## 十一、整改过程中的更正（2026-10-07 补记）

深入代码后发现本评审中有几条判断不准确，特此更正，避免后续维护者按图索骥返工：

### 已证伪的条目

| 原条目 | 结论 | 证据 |
|---|---|---|
| 四.3 文件状态字母（M/A/D）需语义着色 | **已实现** | `web/src/features/source-control/source-control.css` 的 `.git-file-status.tone-{added,deleted,untracked,conflict}` 已用 `--signal/--danger/--ink-soft/--warning` 着色，modified 默认 `--blue` |
| 四.2 diff 统计 `+N/-N` 需色块化 | **已着色** | `file-diff-card.css` 中 `.git-file-card-stats b/i` 已用 `--diff-added-text/--diff-removed-text`，色条只是进一步优化 |
| 四.1 Staged/Unstaged/Untracked 分组 | **已实现** | `changes/change-groups.ts` 提供 `groupGitChanges()`，且支持 `untracked_changes` 配置（mixed/separate/hidden） |
| 八 `.jump-to-bottom` 绝对定位会遮挡 | **误判** | 它是 `.composer-dock` 的子元素，通过 `bottom: calc(100% + 0.35rem)` 把按钮定位在 composer 上方，父级是 sticky，行为正确 |
| 八 徽章最小字号需锁 11px | **风险过大** | `--text-2xs` 在 220 处使用，密集 UI（徽章、角落标记、命令输出）依赖 10px 保持紧凑，一刀切会撑破布局。如需优化应按场景个案处理 |
| 三.1 `--chat-column-min: 34rem` | **不该改** | 当前 `min(100%, 52rem)` 是合理的响应式行为，硬塞最小宽度会导致水平滚动 |

### 修正后的真实剩余项

| 优先级 | 条目 | 状态 |
|---|---|---|
| P0 | sticky diff 卡片头背景 | **已修复**（commit `ad77d15e`） |
| P1 | 工具调用折叠摘要升级（"写入 N 个文件 · 读取 M 次"） | 待做，需要改 `tool-renderers` 聚合逻辑与文案规则，工作量大于 CSS 调整 |
| P2 | 空态引导卡片、运行中工具呼吸动效、Web/TUI 设置语义对齐 | 待做 |

### 教训

下次评审前应先用 `grep` 验证「我以为没做」的条目是否真的没做，再写进文档。

### P1/P2 全部走查完毕的补充（2026-10-07 再补）

继续按计划走查 P1/P2，结论如下：

| 条目 | 结论 | 证据 |
|---|---|---|
| P1 工具调用折叠摘要升级 | **本次完成** | commit `60002dde`，`countWorkItems` 拆分读/写/命令/其他，折叠态显示「写入 N · 读取 M · 命令 K」 |
| P2 空态引导卡片 | **已实现** | `chat-empty-state.tsx` 三个起手式（了解项目 / 审阅变更 / 规划功能），点击填草稿 + 焦点跳转 |
| P2 运行中工具呼吸动效 | **已实现** | `tool-card-shell.css` 的 `tool-shell-breathe` 关键帧 + `prefers-reduced-motion` 兜底 + 运行态头部 4% signal 薄底 |
| P2 Web/TUI 设置语义对齐 | **不建议做** | TUI 是扁平 7 项菜单，Web 是 3 大类 18 分区。TUI 按使用频率分组（高级设置 = 低频收纳），Web 按能力维度分组（智能体能力 / 基础 / 数据）。两种分类学不同，强行对齐会破坏 Web 端的可发现性 |

**最终结论**：原评审中 12 个可执行项里，真实有效的只有 2 项，已全部在本分支落地：
1. sticky diff 卡片头背景穿透修复（commit `ad77d15e`）
2. 工具调用折叠摘要分类（commit `60002dde`）

其余 10 项要么已实现、要么实现方式比提案更好、要么做了反而破坏现有设计。

**评审的真正产出**不是大量改动，而是**确认 Sai 的 UI 完成度比截图看上去高得多**——很多「看着像没做」的痛点，实际已经在代码里解决了，只是截图里看不出。
