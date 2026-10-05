import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import net from 'node:net';
import { spawn } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { _electron as electron } from 'playwright';
import { captureWindow } from './smoke-window-capture.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
if (process.platform !== 'linux') throw new Error('此冒烟脚本通过 Linux XDG 目录隔离真实用户数据，请在 Linux 运行');
const temporary = await mkdtemp(path.join(os.tmpdir(), 'sai-desktop-smoke-'));
const output = path.join(root, 'test-results');
const environment = { ...process.env,
  // 【桌面测试】【AppImage 隔离】免 FUSE 模式复用解压目录，双启动测试结束后统一清理
  TMPDIR: temporary, NO_CLEANUP: '1',
  XDG_CONFIG_HOME: path.join(temporary, 'config'), XDG_DATA_HOME: path.join(temporary, 'data'),
  XDG_STATE_HOME: path.join(temporary, 'state'), XDG_CACHE_HOME: path.join(temporary, 'cache'),
  XDG_PICTURES_DIR: path.join(temporary, 'pictures'), SAI_DESKTOP_WORKSPACE: temporary,
  SAI_BROWSER_STDERR: path.join(temporary, 'browser.log'),
};
delete environment.ELECTRON_RUN_AS_NODE;
await mkdir(output, { recursive: true });
let application;
let blocker;

/**
 * 【桌面测试】【应用启动】在隔离数据目录中启动开发版或安装包
 * @returns {Promise<import('playwright').Page>} 已完成认证的工作台页面
 */
async function launch() {
  application = await electron.launch({
    ...(process.env.SAI_DESKTOP_EXECUTABLE ? { executablePath: path.resolve(process.env.SAI_DESKTOP_EXECUTABLE) } : {}),
    args: [...(process.env.SAI_DESKTOP_EXECUTABLE ? [] : [root]), '--ozone-platform=x11'],
    cwd: temporary, env: environment, timeout: 45000,
  });
  await application.firstWindow({ timeout: 45000 });
  let page;
  for (let attempt = 0; attempt < 300; attempt += 1) {
    page = application.context().pages().find((candidate) => candidate.url().startsWith('http://127.0.0.1:'));
    if (page) break;
    await delay(100);
  }
  assert.ok(page, 'Sai 工作台 WebContentsView 未加载');
  await page.waitForFunction(() => document.querySelector('#root')?.childElementCount > 0);
  await page.waitForFunction(async () => {
    const response = await fetch('/api/workspaces');
    return response.ok;
  }, null, { timeout: 30000 });
  await page.locator('button').first().waitFor({ state: 'visible', timeout: 30000 });
  return page;
}

/**
 * 【桌面测试】【退出验证】关闭应用后检查后端是否停止监听
 * @param {string} origin 后端源地址
 * @returns {Promise<void>} 检查失败时抛出断言错误
 */
async function close(origin) {
  await application.close();
  application = null;
  await assert.rejects(fetch(`${origin}/api/health`, { signal: AbortSignal.timeout(2000) }));
}

try {
  // 1. 【桌面测试】【实际加载】验证内嵌页面、认证接口与 Chromium 隔离设置
  let page = await launch();
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const origin = new URL(page.url()).origin;
  assert.equal((await fetch(`${origin}/api/workspaces`)).status, 401);
  assert.deepEqual(await page.evaluate(() => ({ node: typeof window.require, process: typeof window.process })),
    { node: 'undefined', process: 'undefined' });
  const preferences = await application.evaluate(({ BrowserWindow }) => {
    const settings = BrowserWindow.getAllWindows()[0].webContents.getLastWebPreferences();
    return { sandbox: settings.sandbox, contextIsolation: settings.contextIsolation, nodeIntegration: settings.nodeIntegration };
  });
  assert.deepEqual(preferences, { sandbox: true, contextIsolation: true, nodeIntegration: false });
  const setup = await page.evaluate(async () => {
    const response = await fetch('/api/onboarding/provider', { method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ provider_id: null, display_name: 'Desktop test', base_url: 'http://127.0.0.1:9/v1',
        protocol: 'openai-chat', api_key: 'desktop-test-key', model: 'test-model' }) });
    return response.status;
  });
  assert.equal(setup, 200);
  // 2. 【桌面测试】【单实例】第二次启动立即退出，已有窗口继续可用
  const second = spawn(application.process().spawnfile,
    [...(process.env.SAI_DESKTOP_EXECUTABLE ? [] : [root]), '--ozone-platform=x11'],
    { cwd: temporary, env: environment, stdio: 'ignore' });
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => { second.kill(); reject(new Error('第二个实例未退出')); }, 10000);
    second.once('error', (error) => { clearTimeout(timer); reject(error); });
    second.once('close', (code) => {
      clearTimeout(timer);
      if (code === 0) resolve();
      else reject(new Error(`第二个实例退出码异常：${code}`));
    });
  });
  assert.equal(await application.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows().length), 1);
  await page.reload();
  await page.waitForFunction(async () => (await fetch('/api/workspaces')).ok);
  await page.locator('button').first().waitFor({ state: 'visible', timeout: 30000 });
  await page.evaluate(() => localStorage.setItem('desktop-smoke-persistence', 'saved'));
  const titlebar = application.context().pages().find((candidate) => candidate.url().startsWith('file:'));
  assert.ok(titlebar, '自定义标题栏未加载');
  await titlebar.getByRole('button', { name: '打开内置浏览器', exact: true }).waitFor();
  assert.equal(await page.evaluate(() => typeof window.saiDesktop), 'undefined');
  assert.equal(await application.evaluate(({ Menu }) => Menu.getApplicationMenu()), null);
  const dimensions = await application.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0].getContentSize());
  assert.equal(await page.evaluate(() => innerHeight), dimensions[1] - 40);
  await page.evaluate(() => { document.documentElement.dataset.theme = 'graphite'; });
  await titlebar.waitForFunction(() => getComputedStyle(document.documentElement).getPropertyValue('--ink').trim() === '#ededed');
  await page.evaluate(() => { document.documentElement.dataset.theme = 'linen'; });
  await titlebar.waitForFunction(() => getComputedStyle(document.documentElement).getPropertyValue('--ink').trim() === '#242424');
  await page.locator('.coding-layout').waitFor({ state: 'visible' });
  await captureWindow(application, path.join(output, 'desktop.png'));
  await titlebar.getByRole('button', { name: '打开内置浏览器', exact: true }).click();
  await page.locator('.browser-pane').waitFor({ state: 'visible' });
  await page.waitForFunction(() => document.querySelector('.browser-pane')?.getAttribute('aria-busy') === 'false');
  await captureWindow(application, path.join(output, 'desktop-browser.png'));
  await application.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0].setSize(640, 620));
  await page.waitForFunction(() => innerWidth === 640);
  const positions = await titlebar.getByRole('button').evaluateAll((buttons) => buttons.filter((button) => button.offsetParent)
    .every((button) => button.getBoundingClientRect().right <= innerWidth));
  assert.equal(positions, true);
  await captureWindow(application, path.join(output, 'desktop-compact.png'));
  await application.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows()[0].setSize(1280, 860));

  // 3. 【桌面测试】【导航边界】拦截系统调用后验证外链分流，不打开用户浏览器
  await application.evaluate(({ shell }) => {
    globalThis.smokeExternalUrls = [];
    shell.openExternal = async (url) => { globalThis.smokeExternalUrls.push(url); };
  });
  await page.evaluate(() => {
    window.open('file:///etc/passwd');
    window.open('https://example.com/desktop-test');
  });
  assert.deepEqual(await application.evaluate(() => globalThis.smokeExternalUrls), ['https://example.com/desktop-test']);
  assert.equal(await application.evaluate(({ BrowserWindow }) => BrowserWindow.getAllWindows().length), 1);
  assert.deepEqual(errors, []);
  await close(origin);

  // 4. 【桌面测试】【持久化与端口冲突】重启保留偏好，端口占用时创建新的自有后端
  page = await launch();
  assert.equal(new URL(page.url()).origin, origin);
  assert.equal(await page.evaluate(() => localStorage.getItem('desktop-smoke-persistence')), 'saved');
  await close(origin);
  blocker = net.createServer();
  await new Promise((resolve, reject) => {
    blocker.once('error', reject);
    blocker.listen(Number(new URL(origin).port), '127.0.0.1', resolve);
  });
  page = await launch();
  const fallbackOrigin = new URL(page.url()).origin;
  assert.notEqual(fallbackOrigin, origin);
  await close(fallbackOrigin);
  await new Promise((resolve) => blocker.close(resolve));
  blocker = null;
  const log = await readFile(path.join(environment.XDG_CONFIG_HOME, 'Sai Desktop', 'logs', 'desktop.log'), 'utf8');
  assert.ok(!/token=[A-Za-z0-9_-]+/.test(log));
  await writeFile(path.join(output, 'smoke.json'), `${JSON.stringify({
    passed: true, packaged: Boolean(process.env.SAI_DESKTOP_EXECUTABLE),
    checks: ['页面加载', '自定义标题栏', '主题同步', '视口布局', '窄窗口', '浏览器按钮接入', '令牌认证', '页面隔离', '单实例', '重新加载', '外链分流', '偏好持久化', '端口冲突回退', '后端退出', '日志脱敏'],
  }, null, 2)}\n`);
  console.info('【桌面测试】【验证完成】窗口、认证、端口恢复及后端退出均通过');
} catch (error) {
  process.stderr.write(await readFile(environment.SAI_BROWSER_STDERR, 'utf8').catch(() => ''));
  const workbench = application?.context().pages().find((candidate) => candidate.url().startsWith('http://127.0.0.1:'));
  if (workbench) process.stderr.write(`${await workbench.locator('.browser-pane').textContent().catch(() => '')}\n`);
  process.stderr.write(await readFile(path.join(environment.XDG_CONFIG_HOME, 'Sai Desktop/logs/desktop.log'), 'utf8').then((log) => log.slice(-12000)).catch(() => ''));
  throw error;
} finally {
  await application?.close();
  blocker?.close();
  await rm(temporary, { recursive: true, force: true });
}
