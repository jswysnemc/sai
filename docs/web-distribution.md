# Web 前端独立分发

Sai CLI 不再内嵌前端资源。首次运行 `sai web` 时下载当前后端版本对应的
GitHub Release 资源包；`sai web update` 强制重新下载同一版本，不启动服务。

## 可行性与取舍

前端是 Vite 静态构建产物，已有 Axum 后端独立提供 API，不依赖运行时 Node.js。
把资源来源从二进制替换为本地目录不需要改动前后端协议，方案可行。
改动前本地 `web/dist` 约 43 MB，CLI 可以取消这些资源的内嵌，并取消发布二进制前的
Node.js 构建步骤。最终二进制减少量取决于构建模式与资源压缩，不能直接等同于目录大小。

代价是首次启动 Web 需要访问 GitHub。仅使用终端功能不产生下载；有缓存后可以离线启动。
GitHub 无法访问或版本资源尚未发布时会明确报错，并提示使用本地资源目录。
不使用 `latest`，避免旧后端下载新前端后出现接口不兼容。

## 发布与安装

- Release 工作流独立构建前端，发布 `sai-web.tar.gz` 及 `sai-web.tar.gz.sha256`。
- 包内 `sai-web.json` 记录后端版本；打包时核对前端版本及发布标签。
- 下载地址为 `https://github.com/jswysnemc/sai/releases/download/v<版本>/sai-web.tar.gz`。
- 资源存放在 Sai 数据目录下 `web/v<版本>/<SHA-256>-<安装ID>/`，`current` 指向当前完整资源。
- 下载先验证 SHA-256，再在临时目录解包。解包拒绝父目录、绝对路径、链接和特殊文件，
  并限制下载、解压大小及文件数量。
- 安装完成后原子替换 `current`；下载或校验失败保留旧指针。旧目录继续保留，供运行中的服务使用。
- 更新后重启 Web 服务使用新资源；正在运行的实例固定使用启动时的资源目录。
- 已发布的旧版本若缺少资源包，需要补充对应产物；本地修改发布工作流不会自动补发已有 Release。

## 本地构建与离线部署

CLI 编译只需要 Rust 与平台依赖，无需 `web/dist`。开发前端时遵循 `web/package-lock.json`：

```sh
cd web
npm ci
npm run build
cd ..
SAI_WEB_ASSETS_DIR="$PWD/web/dist" ./target/release/sai web --foreground
```

`SAI_WEB_ASSETS_DIR` 是显式资源目录覆盖，必须包含 `index.html`。
设置此变量时不下载；运行 `sai web update` 会提示取消覆盖后再更新，避免修改开发目录。
Windows 可通过 PowerShell 的 `$env:SAI_WEB_ASSETS_DIR` 设置同一变量。

桌面安装包及 Docker 镜像携带独立静态目录，并设置该变量，因此保留离线启动能力。
发布包可通过 `node web/scripts/package-release.mjs` 生成，默认输出到 `target/web-release/`。
