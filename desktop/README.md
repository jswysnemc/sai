# Sai Desktop

本目录用 Electron 封装本仓库的 React 工作台与 Rust 后端。安装包包含 Chromium、内嵌 Web 工作台的 `sai` 可执行文件和桌面外壳。

定制说明见 [桌面适配层](docs/desktop-adapter.md)。

## 开发与构建

需要 Node.js 22、pnpm、Rust stable，以及 Sai 本身的系统编译依赖。

```bash
cd desktop
pnpm install --frozen-lockfile
pnpm backend:build
pnpm start
```

完整构建安装包：

```bash
pnpm build
```

构建顺序：检查 Sai 接口 → 构建标题栏与浏览器启动器 → 安装 Web 锁定依赖并构建前端 → Cargo release → 将二进制写入 `.staging/<os>-<arch>/` → Electron 打包。

`pnpm pack` 生成可直接运行的未压缩目录，仅供本机验证。`pnpm dist` 复用已准备的后端，并重新构建桌面外壳。安装包会移到 `dist/packages/`；`.staging/`、`linux-unpacked/` 等目录只留在本机，不进入发布产物。

## 平台产物

| 构建主机 | 默认产物 |
| --- | --- |
| Linux | AppImage、tar.gz |
| macOS | DMG、ZIP |
| Windows | NSIS 安装程序、ZIP |

请在目标系统及目标架构构建后端，再运行 Electron 打包。打包前会验证 ELF、PE 或 Mach-O 文件头与 SHA-256。本地打包不发布产物，macOS 使用本地临时签名。

```bash
pnpm build:win
pnpm build:mac --arch arm64
pnpm build:linux --arch x64
```

交叉编译仍要求先构建当前前端资源；导入流程只能确认文件格式与校验值。

## 运行行为

- 启动独立的 Sai 后端，只监听 `127.0.0.1`。
- 保留 Sai 启动令牌认证。
- 使用原有 Sai 配置、会话、工作区和密钥目录。
- 内置浏览器由独立 Electron 进程运行。
- 自定义标题栏跟随 Sai 主题。

## 验证

```bash
pnpm test
pnpm desktop:check
xvfb-run -a pnpm test:smoke
xvfb-run -a pnpm test:browser
```
