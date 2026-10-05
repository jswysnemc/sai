import assert from 'node:assert/strict';
import http from 'node:http';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { setTimeout as delay } from 'node:timers/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { WebSocket } from 'ws';
import backendModule from '../electron/backend.cjs';
import cdpModule from '../tests/support/cdp-client.cjs';

const require = createRequire(import.meta.url);
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
if (process.platform !== 'linux') throw new Error('此脚本使用 Linux XDG 临时目录隔离真实配置');
const temporary = await mkdtemp(path.join(os.tmpdir(), 'sai-electron-browser-'));
const packagedExecutable = process.env.SAI_DESKTOP_EXECUTABLE && path.resolve(process.env.SAI_DESKTOP_EXECUTABLE);
const resourcesPath = packagedExecutable ? path.join(path.dirname(packagedExecutable), 'resources') : undefined;
const environment = { ...process.env, XDG_CONFIG_HOME: path.join(temporary, 'config'),
  XDG_DATA_HOME: path.join(temporary, 'data'), XDG_STATE_HOME: path.join(temporary, 'state'),
  XDG_CACHE_HOME: path.join(temporary, 'cache'), XDG_PICTURES_DIR: path.join(temporary, 'pictures'),
  SAI_BROWSER_PROFILE: path.join(temporary, 'profile'), SAI_BROWSER_STDERR: path.join(temporary, 'browser.log'),
  SAI_BROWSER_EXECUTABLE: packagedExecutable ? path.join(resourcesPath, 'desktop-host/sai-browser-launcher')
    : path.join(root, `.staging/linux-${process.arch}/desktop-host/sai-browser-launcher`),
  SAI_DESKTOP_EXECUTABLE: packagedExecutable || require('electron') };
if (packagedExecutable) delete environment.SAI_DESKTOP_DEV_ENTRY;
else environment.SAI_DESKTOP_DEV_ENTRY = root;
delete environment.ELECTRON_RUN_AS_NODE;
const backend = new backendModule.Backend({
  executable: backendModule.backendPath({ packaged: Boolean(packagedExecutable), resourcesPath, projectRoot: root }), cwd: temporary,
  env: environment, logger: { info() {}, error: (line) => process.stderr.write(`${line}\n`) },
});
let socket;
let cdp;
let state;
let failure;
let frames = 0;
const downloads = [];
const fixture = http.createServer((request, response) => {
  if (request.url === '/download') {
    response.writeHead(200, { 'Content-Type': 'text/plain', 'Content-Disposition': 'attachment; filename="fixture.txt"' });
    response.end('electron-browser-download');
    return;
  }
  response.setHeader('Content-Type', 'text/html; charset=utf-8');
  response.end('<!doctype html><title>Browser fixture</title><style>body{background:#f4f4f4;color:#222;font:16px sans-serif;padding:24px}input{font:inherit}</style><h1>Electron browser fixture</h1><input id="entry"><a id="download" href="/download" download>Download</a>');
});

/**
 * 【桌面测试】【状态等待】等待浏览器协议达到指定状态，失败时保留明确原因
 * @param {Function} predicate 状态判断函数
 * @param {string} label 失败说明
 * @returns {Promise<void>} 条件满足时完成
 */
async function until(predicate, label) {
  for (let attempt = 0; attempt < 300; attempt += 1) {
    if (failure) throw new Error(failure);
    if (await predicate()) return;
    await delay(100);
  }
  throw new Error(`等待失败：${label}`);
}

/**
 * 【桌面测试】【面板连接】创建实际 Sai 浏览器连接，记录画面和状态
 * @param {URL} url Sai 后端地址
 * @param {string} cookie 测试登录 Cookie
 * @returns {Promise<void>} 初始标签就绪后完成
 */
async function openPanel(url, cookie) {
  state = undefined;
  socket = new WebSocket(`${url.origin.replace('http:', 'ws:')}/api/browser/socket`, { headers: { Cookie: cookie } });
  socket.on('error', (error) => { failure = error.message; });
  socket.on('message', (data, binary) => {
    if (binary) { frames += 1; return; }
    const message = JSON.parse(data);
    if (message.type === 'state') state = message.state;
    if (message.type === 'error') failure = message.message;
    if (message.type === 'download') downloads.push(message.download);
  });
  await until(() => state?.tabs?.length === 1, '初始标签');
}

/**
 * 【桌面测试】【协议接入】通过 Sai 管理的用户目录连接浏览器并附着当前页面
 * @returns {Promise<object>} 调试端口、地址及页面会话
 */
async function attach() {
  const [port, endpoint] = (await readFile(path.join(environment.SAI_BROWSER_PROFILE, 'DevToolsActivePort'), 'utf8')).trim().split('\n');
  const address = `ws://127.0.0.1:${port}${endpoint}`;
  cdp = new cdpModule.CdpClient(address);
  const targets = await cdp.send('Target.getTargets');
  const target = targets.targetInfos.find((item) => item.url === state.url);
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId: target.targetId, flatten: true });
  return { port, address, sessionId };
}

/**
 * 【桌面测试】【浏览器关闭】验证浏览器完全退出且工作台后端继续提供服务
 * @param {URL} url Sai 后端地址
 * @param {string} cookie 测试登录 Cookie
 * @param {string} port 当前浏览器调试端口
 * @returns {Promise<void>} 浏览器退出后完成
 */
async function closeBrowser(url, cookie, port) {
  socket.close();
  cdp.close();
  const closed = await fetch(`${url.origin}/api/browser/close`, { method: 'POST', headers: { Cookie: cookie } });
  assert.equal(closed.status, 200);
  assert.equal((await fetch(`${url.origin}/api/health`)).status, 200);
  await until(async () => !await fetch(`http://127.0.0.1:${port}/json/version`).then(() => true, () => false), '浏览器进程退出');
}

try {
  await new Promise((resolve) => fixture.listen(0, '127.0.0.1', resolve));
  const fixtureUrl = `http://127.0.0.1:${fixture.address().port}`;
  const url = await backend.start();
  const login = await fetch(`${url.origin}/api/auth/session?${url.searchParams}`, { method: 'POST' });
  const cookie = login.headers.get('set-cookie').split(';')[0];
  // 1. 【桌面测试】【Sai 接入】由原生后端启动 Electron，保持浏览器面板协议不变
  await openPanel(url, cookie);
  socket.send(JSON.stringify({ type: 'navigate', url: `${fixtureUrl}/one` }));
  await until(() => state.url === `${fixtureUrl}/one` && frames > 0, '导航及浏览画面');
  const firstTab = state.tabs[0].id;
  let { port, address, sessionId } = await attach();
  const evaluate = async (expression) => (await cdp.send('Runtime.evaluate', { expression, returnByValue: true }, sessionId)).result.value;
  assert.equal(await evaluate('typeof window.require'), 'undefined');
  assert.equal(await evaluate('typeof window.saiDesktop'), 'undefined');
  assert.ok(!(await evaluate('navigator.userAgent')).includes('Electron/'));
  await assert.rejects(new Promise((resolve, reject) => {
    const hostile = new WebSocket(address, { origin: 'https://untrusted.example', handshakeTimeout: 5000 });
    hostile.once('open', () => { hostile.close(); resolve(); });
    hostile.once('error', reject);
  }), /socket hang up|Unexpected server response/);
  // 2. 【桌面测试】【输入与标签】面板输入、Agent CDP 操作和多标签共用同一页面
  await evaluate('document.getElementById("entry").focus()');
  socket.send(JSON.stringify({ type: 'insert_text', text: '桌面浏览器输入' }));
  await until(async () => await evaluate('document.getElementById("entry").value') === '桌面浏览器输入', '输入同步');
  socket.send(JSON.stringify({ type: 'new_tab' }));
  await until(() => state.tabs.length === 2, '新建标签');
  socket.send(JSON.stringify({ type: 'switch_tab', id: firstTab }));
  await until(() => state.url === `${fixtureUrl}/one`, '切换标签');
  await evaluate(`void window.open(${JSON.stringify(`${fixtureUrl}/popup`)})`);
  await until(() => state.tabs.length === 3 && state.url === `${fixtureUrl}/popup`, '弹出页接管');
  socket.send(JSON.stringify({ type: 'switch_tab', id: firstTab }));
  await until(() => state.url === `${fixtureUrl}/one`, '返回原页面');
  // 3. 【桌面测试】【下载与隔离】通过原 Sai 下载接口读取 Electron 保存的文件
  await evaluate('document.getElementById("download").click()');
  await until(() => downloads.some((item) => item.state === 'completed'), '下载完成');
  const download = downloads.find((item) => item.state === 'completed');
  const file = await fetch(`${url.origin}/api/browser/downloads/${download.guid}`, { headers: { Cookie: cookie } });
  assert.equal(await file.text(), 'electron-browser-download');
  // 4. 【桌面测试】【浏览数据】持久 Cookie 跨进程保留，原 Sai 清除接口删除 Electron 会话
  await evaluate('document.cookie = "desktop_persistent=retained; Max-Age=3600; Path=/"');
  await closeBrowser(url, cookie, port);
  await openPanel(url, cookie);
  socket.send(JSON.stringify({ type: 'navigate', url: `${fixtureUrl}/one` }));
  await until(() => state.url === `${fixtureUrl}/one`, '重启后导航');
  ({ port, sessionId } = await attach());
  assert.ok((await evaluate('document.cookie')).includes('desktop_persistent=retained'));
  await closeBrowser(url, cookie, port);
  const cleared = await fetch(`${url.origin}/api/browser/clear-data`, { method: 'POST', headers: { Cookie: cookie } });
  assert.equal(cleared.status, 200);
  await openPanel(url, cookie);
  socket.send(JSON.stringify({ type: 'navigate', url: `${fixtureUrl}/one` }));
  await until(() => state.url === `${fixtureUrl}/one`, '清除后导航');
  ({ port, sessionId } = await attach());
  assert.ok(!(await evaluate('document.cookie')).includes('desktop_persistent='));
  await closeBrowser(url, cookie, port);
  await mkdir(path.join(root, 'test-results'), { recursive: true });
  await writeFile(path.join(root, 'test-results/browser-smoke.json'), `${JSON.stringify({ passed: true,
    packaged: Boolean(packagedExecutable),
    checks: ['Sai 启动 Electron', '画面传输', '输入同步', '标签管理', '弹出页', '下载接口', '页面隔离', 'Origin 校验', '独立退出', 'Cookie 持久化', '清除浏览数据'] }, null, 2)}\n`);
  console.info('【桌面浏览器】【验证通过】Sai 协议、标签、输入、下载与退出均通过');
} catch (error) {
  process.stderr.write(await readFile(environment.SAI_BROWSER_STDERR, 'utf8').catch(() => ''));
  throw error;
} finally {
  socket?.close();
  cdp?.close();
  await backend.stop();
  fixture.closeAllConnections();
  await new Promise((resolve) => fixture.close(resolve));
  await rm(temporary, { recursive: true, force: true });
}
