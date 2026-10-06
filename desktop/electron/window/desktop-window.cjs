const { BrowserWindow, WebContentsView, Menu, screen } = require('electron');
const path = require('node:path');
const { configureWorkbench } = require('./workbench-policy.cjs');
const { installControls } = require('./window-controls.cjs');
const { createChromeLayout, CONTROLS_WIDTH } = require('./window-chrome.cjs');

const MAC = process.platform === 'darwin';

/**
 * 【桌面端】【一体化窗口】工作台铺满整个窗口，窗口按钮叠在工作台自身的顶行上
 *
 * Windows/Linux 在右上角放一个只含最小化、最大化、关闭的小视图；
 * macOS 使用系统红绿灯，不创建按钮视图。工作台通过预加载脚本得知需要让出的宽度。
 * @param {URL} url Sai 后端启动地址
 * @param {object} logger 日志接口
 * @returns {Promise<BrowserWindow>} 已加载的桌面窗口
 */
async function createWindow(url, logger) {
  const { width, height } = screen.getPrimaryDisplay().workAreaSize;
  const window = new BrowserWindow({
    width: Math.min(1280, width), height: Math.min(860, height),
    minWidth: Math.min(520, width), minHeight: Math.min(420, height),
    show: false, title: 'Sai Desktop', backgroundColor: '#f7f7f7',
    icon: path.join(__dirname, '../../build/icon.png'),
    // 1. macOS 保留系统红绿灯并让它落在工作台顶行内；其他平台完全无边框
    ...(MAC ? { titleBarStyle: 'hiddenInset', trafficLightPosition: { x: 12, y: 9 } } : { frame: false }),
    webPreferences: { nodeIntegration: false, contextIsolation: true, sandbox: true, webviewTag: false },
  });
  const view = new WebContentsView({ webPreferences: {
    partition: 'persist:sai-desktop', preload: path.join(__dirname, '../preload/workbench.cjs'),
    nodeIntegration: false, contextIsolation: true, sandbox: true, webSecurity: true, webviewTag: false,
  } });
  window.contentView.addChildView(view);
  // 2. 按钮视图后加入，叠在工作台之上
  const controls = MAC ? null : new WebContentsView({ webPreferences: {
    preload: path.join(__dirname, '../preload/shell.cjs'),
    nodeIntegration: false, contextIsolation: true, sandbox: true, webviewTag: false,
  } });
  if (controls) {
    controls.setBackgroundColor('#00000000');
    window.contentView.addChildView(controls);
  }
  const chrome = createChromeLayout(window, view, controls);
  window.setMenu(null);
  Menu.setApplicationMenu(MAC ? Menu.buildFromTemplate([{ role: 'appMenu' }, { role: 'editMenu' }]) : null);
  for (const contents of [window.webContents, controls?.webContents].filter(Boolean)) {
    contents.setWindowOpenHandler(() => ({ action: 'deny' }));
    contents.on('will-navigate', (event) => event.preventDefault());
  }
  if (controls) {
    const session = controls.webContents.session;
    session.setPermissionCheckHandler(() => false);
    session.setPermissionRequestHandler((_contents, _permission, callback) => callback(false));
    session.setDevicePermissionHandler(() => false);
  }
  await configureWorkbench(view.webContents, url, logger);
  const cleanup = installControls(window, view.webContents, controls?.webContents ?? null, url.origin, chrome);
  window.once('closed', () => {
    cleanup();
    for (const child of [view, controls].filter(Boolean)) {
      if (!child.webContents.isDestroyed()) child.webContents.close();
    }
  });
  if (controls) await controls.webContents.loadFile(path.join(__dirname, '../../renderer/dist/index.html'));
  window.show();
  await view.webContents.loadURL(url.href);
  view.webContents.focus();
  return window;
}

module.exports = { createWindow, CONTROLS_WIDTH };
