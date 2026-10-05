const path = require('node:path');
const fs = require('node:fs');

/**
 * 【桌面适配】【浏览器注入】仅为 Sai 子进程设置已有扩展点，不改变系统环境
 * @param {App} app Electron 应用实例
 * @returns {object} 后端子进程环境变量
 */
function browserEnvironment(app) {
  const platform = process.platform === 'win32' ? 'win' : process.platform === 'darwin' ? 'mac' : 'linux';
  const binary = process.platform === 'win32' ? 'sai-browser-launcher.exe' : 'sai-browser-launcher';
  const directory = app.isPackaged ? path.join(process.resourcesPath, 'desktop-host')
    : path.join(app.getAppPath(), '.staging', `${platform}-${process.arch}`, 'desktop-host');
  const executable = path.join(directory, binary);
  if (!fs.existsSync(executable)) throw new Error('缺少桌面浏览器启动器，请运行 pnpm desktop:prepare');
  const environment = { ...process.env, SAI_BROWSER_EXECUTABLE: executable,
    SAI_DESKTOP_EXECUTABLE: process.execPath };
  if (!app.isPackaged) environment.SAI_DESKTOP_DEV_ENTRY = app.getAppPath();
  else delete environment.SAI_DESKTOP_DEV_ENTRY;
  delete environment.ELECTRON_RUN_AS_NODE;
  return environment;
}

module.exports = { browserEnvironment };
