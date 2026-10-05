const { app, BrowserWindow, dialog, shell } = require('electron');
const path = require('node:path');
const fs = require('node:fs');
const { Backend, backendPath } = require('./backend.cjs');
const { createWindow } = require('./window/desktop-window.cjs');
const { createLogger, redact } = require('./logging.cjs');
const { readPort, savePort } = require('./runtime-state.cjs');
const { browserEnvironment } = require('./browser-host/environment.cjs');

app.setName('Sai Desktop');
app.setAppUserModelId('com.sai.desktop');
app.commandLine.appendSwitch('class', 'sai-desktop');

let backend;
let logger;
let quitting = false;
let canQuit = false;
let handlingFailure = false;

/**
 * 【桌面端】【错误处理】停止后端，展示可重启的错误信息
 * @param {Error} error 启动或运行错误
 * @returns {Promise<void>} 用户完成选择后退出或重启
 */
async function fail(error) {
  if (quitting || handlingFailure) return;
  handlingFailure = true;
  logger?.error(`【桌面端】【运行失败】${error.stack || error.message}`);
  await backend?.stop().catch((stopError) => logger?.error(`【桌面端】【清理失败】${stopError.message}`));
  const result = await dialog.showMessageBox({
    type: 'error', title: 'Sai Desktop', message: '工作台暂时无法运行',
    detail: `${redact(error.message)}\n\n日志位置：${logger?.file || app.getPath('logs')}`,
    buttons: ['退出', '重新启动', '打开日志目录'], defaultId: 1, cancelId: 0,
  });
  if (result.response === 2) await shell.openPath(app.getPath('logs'));
  if (result.response === 1) app.relaunch();
  app.quit();
}

/**
 * 【桌面端】【启动入口】启动随包后端并打开工作台
 * @returns {Promise<void>} 主窗口加载完成后返回
 */
async function start() {
  app.setPath('logs', path.join(app.getPath('userData'), 'logs'));
  logger = createLogger(app.getPath('logs'));
  const cwd = path.resolve(process.env.SAI_DESKTOP_WORKSPACE || app.getPath('home'));
  if (!fs.statSync(cwd).isDirectory()) throw new Error(`工作目录不存在：${cwd}`);
  const options = {
    executable: backendPath({
      packaged: app.isPackaged, resourcesPath: process.resourcesPath, projectRoot: path.join(__dirname, '..'),
    }), cwd, workspace: process.env.SAI_DESKTOP_WORKSPACE ? cwd : undefined, logger, onExit: fail,
    env: browserEnvironment(app),
  };
  const stateFile = path.join(app.getPath('userData'), 'runtime.json');
  const port = readPort(stateFile);
  backend = new Backend({ ...options, port });
  let url;
  try {
    url = await backend.start();
  } catch (error) {
    if (!port || quitting) throw error;
    // 1. 【桌面端】【端口回退】旧端口不可用时另起服务，绝不连接占用端口的其他进程
    logger.info('【桌面端】【端口回退】改用新的本机端口');
    backend = new Backend(options);
    url = await backend.start();
  }
  savePort(stateFile, Number(url.port));
  if (!quitting) await createWindow(url, logger);
}

// 1. 【桌面端】【单实例】重复启动只激活已有窗口，避免重复启动后端
if (!app.requestSingleInstanceLock()) {
  app.quit();
} else {
  app.on('second-instance', () => {
    const window = BrowserWindow.getAllWindows()[0];
    if (window) { if (window.isMinimized()) window.restore(); window.show(); window.focus(); }
  });
  app.whenReady().then(start).catch(fail);
  app.on('window-all-closed', () => app.quit());
  // 2. 【桌面端】【退出回收】等待后端资源释放完成后再结束主进程
  app.on('before-quit', (event) => {
    if (canQuit) return;
    event.preventDefault();
    if (quitting) return;
    quitting = true;
    Promise.resolve(backend?.stop()).catch((error) => {
      logger?.error(`【桌面端】【退出失败】${error.message}`);
    }).finally(() => { canQuit = true; app.quit(); });
  });
  process.on('SIGINT', () => app.quit());
  process.on('SIGTERM', () => app.quit());
  process.on('uncaughtException', (error) => { void fail(error); });
  process.on('unhandledRejection', (error) => { void fail(error instanceof Error ? error : new Error(String(error))); });
}
