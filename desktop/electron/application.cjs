const { app, BrowserWindow, dialog, shell } = require('electron');
const path = require('node:path');
const fs = require('node:fs');
const { Backend, backendPath } = require('./backend.cjs');
const { createWindow } = require('./window/desktop-window.cjs');
const { createLogger, redact } = require('./logging.cjs');
const { readPort, savePort } = require('./runtime-state.cjs');
const { browserEnvironment } = require('./browser-host/environment.cjs');

// ===== 新增 import =====
const { workspaceFromLaunch } = require('./workspace-launch.cjs');

app.setName('Sai Desktop');
app.setAppUserModelId('com.sai.desktop');
app.commandLine.appendSwitch('class', 'sai-desktop');

let backend;
let logger;
let quitting = false;
let canQuit = false;
let handlingFailure = false;
let serviceUrl = null;

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
  const launchWorkspace = workspaceFromLaunch(process.argv, launchContext());
  const cwd = path.resolve(process.env.SAI_DESKTOP_WORKSPACE || launchWorkspace || app.getPath('home'));
  if (!fs.statSync(cwd).isDirectory()) throw new Error(`工作目录不存在：${cwd}`);
  const options = {
    executable: backendPath({
      packaged: app.isPackaged, resourcesPath: process.resourcesPath, projectRoot: path.join(__dirname, '..'),
    }), cwd, workspace: process.env.SAI_DESKTOP_WORKSPACE || launchWorkspace ? cwd : undefined, logger, onExit: fail,
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
  serviceUrl = url;
  if (!quitting) await createWindow(url, logger);
}

// 1. 【桌面端】【单实例】重复启动只激活已有窗口，避免重复启动后端
if (!app.requestSingleInstanceLock()) {
  app.quit();
} else {
  app.on('second-instance', (_event, argv) => {
    const window = BrowserWindow.getAllWindows()[0];
    if (window) { if (window.isMinimized()) window.restore(); window.show(); window.focus(); }
    void openLaunchedWorkspace(argv);
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

/**
 * 【桌面端】【启动目录】判断参数是不是本机目录。
 * @returns {{ execPath: string, appPath: string, isDirectory: (candidate: string) => boolean }} 启动参数上下文
 */
function launchContext() {
  return {
    execPath: process.execPath,
    appPath: app.getAppPath(),
    isDirectory(candidate) {
      try { return fs.statSync(candidate).isDirectory(); }
      catch { return false; }
    },
  };
}

/**
 * 【桌面端】【打开工作区】把右键选中的目录登记为活动工作区并刷新工作台。
 * @param {string[]} argv 第二次启动的参数
 * @returns {Promise<void>} 切换完成后返回
 */
async function openLaunchedWorkspace(argv) {
  const directory = workspaceFromLaunch(argv, launchContext());
  if (!directory || !serviceUrl) return;
  const token = serviceUrl.searchParams.get('token');
  const headers = { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` };
  try {
    // 1. 先登记目录，已存在时接口返回原来的工作区
    const added = await fetch(`${serviceUrl.origin}/api/workspaces`, {
      method: 'POST', headers, body: JSON.stringify({ path: directory }),
    });
    if (!added.ok) throw new Error(`登记失败 ${added.status}`);
    const workspace = await added.json();
    // 2. 有终端时先关掉再切换，否则右键打开停在旧目录
    let switched = await fetch(`${serviceUrl.origin}/api/workspaces/${encodeURIComponent(workspace.id)}/switch`, {
      method: 'POST', headers,
    });
    if (switched.status === 409) {
      switched = await fetch(`${serviceUrl.origin}/api/workspaces/${encodeURIComponent(workspace.id)}/switch?close_terminals=true`, {
        method: 'POST', headers,
      });
    }
    if (!switched.ok) throw new Error(`切换失败 ${switched.status}`);
    const page = require('electron').webContents.getAllWebContents()
      .find((contents) => contents.getURL().startsWith(serviceUrl.origin));
    page?.reload();
  } catch (error) {
    logger?.error(`【桌面端】【打开工作区】${error.message}`);
  }
}
