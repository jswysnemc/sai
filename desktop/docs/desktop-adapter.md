# 桌面适配层

桌面定制位于本仓库 `desktop/`。Sai 继续维护工作台、浏览器面板、业务接口及 Rust 后端；该目录负责窗口、标题栏、Electron 浏览器宿主与安装包，不复制业务源文件。

## 文件边界

```text
desktop/
├── electron/
│   ├── main.cjs                     # 区分桌面入口与浏览器进程入口
│   ├── application.cjs              # 桌面应用生命周期及后端环境
│   ├── window/                      # 无边框窗口、控件与工作台策略
│   ├── preload/                     # 标题栏 API 与工作台适配
│   └── browser-host/                # Electron 浏览器及 CDP 适配
├── renderer/                        # 独立 React 标题栏
├── native/browser-launcher/         # 跨平台 Rust 浏览器启动器
├── scripts/                         # 构建、校验与集成测试
└── .staging/<os>-<arch>/            # 本地暂存，不进入 Git 与发布产物
```

构建时后端与启动器写入 `.staging/`。electron-builder 只把对应平台的二进制复制进安装包的 `resources/backend` 与 `resources/desktop-host`。安装包随后移到 `dist/packages/`；解压目录、暂存目录和校验过程文件都不作为发布文件上传。

## 窗口与标题栏

主窗口使用 `frame: false`。40px 标题栏由独立渲染页面提供，Sai 工作台放在下方的 `WebContentsView` 中，继续使用自身的视口、路由与响应式布局。桌面样式不会注入 Sai DOM。

标题栏支持前进、后退、刷新、终端、浏览器、帮助与窗口控制。窄窗口隐藏次要控件。Windows/Linux 不显示原生菜单；macOS 保留系统菜单栏中的应用与编辑菜单。

工作台预加载脚本只发送主题变量，并接收固定面板动作。窗口 IPC 仅接受标题栏主框架，业务页面与浏览网页没有 `window.saiDesktop`。两类页面均关闭 Node 集成并启用沙盒、上下文隔离。

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
| `data-theme` 与主题 CSS 变量 | 同步标题栏配色 |
