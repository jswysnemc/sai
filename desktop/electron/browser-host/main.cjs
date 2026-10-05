const { app, Menu, session } = require('electron');
const fs = require('node:fs/promises');
const syncFs = require('node:fs');
const path = require('node:path');
const { setTimeout: delay } = require('node:timers/promises');
const { BrowserTabs } = require('./browser-tabs.cjs');
const { BrowserDownloads } = require('./browser-downloads.cjs');
const { startCdpServer } = require('./cdp-server.cjs');

/**
 * 【桌面浏览器】【参数读取】读取 Sai 提供的 Chrome 风格启动参数
 * @param {string} name 参数名称，不含前导横线
 * @returns {string|undefined} 参数值
 */
function argument(name) {
  const prefix = `--${name}=`;
  const joined = process.argv.find((value) => value.startsWith(prefix));
  if (joined) return joined.slice(prefix.length);
  const index = process.argv.indexOf(`--${name}`);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

/**
 * 【桌面浏览器】【原生端点】等待 Electron 写入自己的 CDP 端点
 * @param {string} directory 独立 Chromium 会话目录
 * @returns {Promise<string>} 浏览器级 WebSocket 地址
 */
async function nativeEndpoint(directory) {
  for (let attempt = 0; attempt < 150; attempt += 1) {
    const text = await fs.readFile(path.join(directory, 'DevToolsActivePort'), 'utf8').catch(() => '');
    const [port, endpoint] = text.trim().split('\n');
    if (/^\d+$/.test(port) && endpoint?.startsWith('/devtools/browser/')) return `ws://127.0.0.1:${port}${endpoint}`;
    await delay(100);
  }
  throw new Error('Electron 调试端点未就绪');
}

/**
 * 【桌面浏览器】【运行入口】启动独立浏览器，保持 Sai 的用户目录、CDP 和关闭约定
 * @returns {Promise<void>} 协议服务就绪后完成
 */
async function startBrowserHost() {
  const profile = argument('user-data-dir');
  if (!profile || !path.isAbsolute(profile)) throw new Error('缺少浏览器用户目录');
  const nativeData = path.join(profile, 'electron-session');
  syncFs.mkdirSync(nativeData, { recursive: true });
  syncFs.rmSync(path.join(profile, 'DevToolsActivePort'), { force: true });
  syncFs.rmSync(path.join(nativeData, 'DevToolsActivePort'), { force: true });
  app.setName('Sai Browser');
  app.setPath('userData', profile);
  app.setPath('sessionData', nativeData);
  app.commandLine.appendSwitch('remote-debugging-port', '0');
  app.commandLine.appendSwitch('remote-debugging-address', '127.0.0.1');
  // 1. 【桌面浏览器】【进程隔离】不申请桌面单实例锁，不使用桌面登录会话
  await app.whenReady();
  app.dock?.hide();
  Menu.setApplicationMenu(null);
  const browserSession = session.defaultSession;
  const userAgent = argument('user-agent') || browserSession.getUserAgent().replace(/\s(?:Electron|Sai Browser)\/[^\s]+/g, '');
  browserSession.setUserAgent(userAgent);
  browserSession.setPermissionRequestHandler((_contents, permission, callback) => callback(permission === 'clipboard-sanitized-write'));
  browserSession.setPermissionCheckHandler((_contents, permission) => permission === 'clipboard-sanitized-write');
  const tabs = new BrowserTabs(browserSession, userAgent);
  const downloads = new BrowserDownloads(browserSession);
  tabs.create();
  const upstreamUrl = await nativeEndpoint(nativeData);
  const bridge = await startCdpServer({ upstreamUrl, tabs, downloads, quit: () => app.quit() });
  // 2. 【桌面浏览器】【协议发布】Sai 只看到兼容入口，原生 CDP 端点留在内部会话目录
  const portFile = path.join(profile, 'DevToolsActivePort');
  await fs.writeFile(`${portFile}.tmp`, `${bridge.port}\n${bridge.browserPath}\n`, { mode: 0o600 });
  await fs.rename(`${portFile}.tmp`, portFile);
  const parent = Number(process.env.SAI_DESKTOP_LAUNCHER_PID);
  const monitor = setInterval(() => {
    if (!Number.isSafeInteger(parent) || parent <= 0) return;
    try { process.kill(parent, 0); } catch { app.quit(); }
  }, 1000);
  monitor.unref();
  app.on('window-all-closed', () => {});
  let cleaned = false;
  // 3. 【桌面浏览器】【资源回收】退出时断开协议连接、销毁标签并清理当前端口文件
  app.on('before-quit', () => {
    if (cleaned) return;
    cleaned = true;
    clearInterval(monitor);
    bridge.close();
    tabs.closeAll();
    browserSession.flushStorageData();
    void fs.rm(portFile, { force: true });
  });
}

module.exports = { startBrowserHost };
