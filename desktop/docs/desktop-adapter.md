# 桌面适配层

桌面定制位于本仓库 `desktop/`。Sai 继续维护工作台、浏览器面板、业务接口及 Rust 后端；该目录负责窗口、标题栏、Electron 浏览器宿主与安装包，不复制业务源文件。

## 文件边界

```text
desktop/
├── electron/
│   ├── main.cjs                     # 区分桌面入口与浏览器进程入口
│   ├── application.cjs              # 桌面应用生命周期及后端环境
│   ├── window/                      # 无边框窗口、控件与工作台策略
│   ├── preload/                     # 窗口按钮 API 与工作台适配
│   └── browser-host/                # Electron 浏览器及 CDP 适配
├── renderer/                        # 窗口按钮（最小化、最大化、关闭）
├── native/browser-launcher/         # 跨平台 Rust 浏览器启动器
├── scripts/                         # 构建、校验与集成测试
└── .staging/<os>-<arch>/            # 本地暂存，不进入 Git 与发布产物
```

构建时后端与启动器写入 `.staging/`。electron-builder 只把对应平台的二进制复制进安装包的 `resources/backend` 与 `resources/desktop-host`。安装包随后移到 `dist/packages/`；解压目录、暂存目录和校验过程文件都不作为发布文件上传。

## 窗口与标题栏

桌面端不再有独立标题栏，Sai 工作台的 `WebContentsView` 铺满整个窗口，工作台自己的 2rem 顶行（侧栏标题、会话标题、工作区页签、设置顶栏）就是窗口标题栏：

- Windows/Linux 使用 `frame: false`，右上角叠加一个与顶行等高的透明 `WebContentsView`，只放最小化、最大化与关闭三个按钮。
- macOS 使用 `titleBarStyle: 'hiddenInset'`，红绿灯落在顶行左端，不创建按钮视图；系统菜单栏保留应用与编辑菜单。
- 工作台预加载脚本把平台写入 `<html data-desktop>`，并把需要让出的宽度写入 `--desktop-inset-right` / `--desktop-inset-left`。Web 端 `features/desktop/` 据此给贴着按钮的顶行补内边距，背景色延伸到按钮下方，视觉上是同一条标题栏；侧栏收起、面板交换或换页后自动重新计算。
- 顶行空白处可拖动窗口，按钮、页签、输入框及浮层保持可点击；Linux 双击顶行空白切换最大化。
- 按钮视图高度跟随工作台的 `--toolbar-height` 与缩放比例，Ctrl+加减号缩放时同步调整。

原标题栏上的前进、后退、帮助、终端与浏览器入口已移除：导航由工作台自身完成，终端与浏览器在工作台工具栏和面板中打开，刷新保留 `Ctrl+R`。

窗口 IPC 只接受按钮视图的主框架；工作台只能上报主题、顶行高度与双击最大化，不能调用其他窗口能力。业务页面与浏览网页没有 `window.saiDesktop`。所有页面关闭 Node 集成并启用沙盒、上下文隔离。

## Electron 浏览器

Sai 通过现有 `SAI_BROWSER_EXECUTABLE` 调用随包 Rust 启动器。启动器使用同一 Electron 可执行文件的 `--sai-browser-host` 入口，传递 Sai 的用户目录与调试参数。该进程不申请桌面单实例锁，不创建工作台窗口，也不会关闭桌面后端。

浏览页面运行在独立 Electron 进程的离屏 `BrowserWindow` 中。Sai 原有浏览器面板继续接收 CDP 画面并发送输入。Electron 与 Chrome 的浏览器级 CDP 命令存在差异；适配层管理 `Target` 创建、枚举、激活和关闭，以及下载策略、事件和 `Browser.close`。

默认浏览数据保存在 Sai 浏览器目录的 `electron-session/` 子目录。调试服务只监听 `127.0.0.1`。

## 上游接口

| Sai 接口 | 桌面用途 |
| --- | --- |
| `SAI_BROWSER_EXECUTABLE` | 替换浏览器启动入口 |
| `--user-data-dir`、`--remote-debugging-port` | 接收用户目录与调试启动约定 |
| `<profile>/DevToolsActivePort` | 发布 Sai 可连接的 CDP 地址 |
| `SAI_BROWSER_PROFILE`、关闭与清除 API | 沿用数据保留和清理行为 |
| `sai:open-workspace-panel`，`detail.tab` | 打开原有 `browser` / `terminal` 面板 |
| `data-theme` 与主题 CSS 变量 | 同步窗口按钮配色 |
| `--toolbar-height`、`features/desktop/` | 顶行高度与窗口按钮让位 |
