const { test } = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { Backend } = require('../electron/backend.cjs');
const { parseStartupUrl, checkHealth } = require('../electron/backend-protocol.cjs');

/**
 * 【桌面测试】【后端替身】使用真实子进程验证生命周期
 * @param {string} mode 服务行为模式
 * @param {object} overrides 可选配置覆盖
 * @returns {Backend} 可供测试的管理实例
 */
function fixture(mode = 'ready', overrides = {}) {
  return new Backend({
    executable: process.execPath, prefixArgs: [path.join(__dirname, 'fixtures', 'backend.cjs')],
    cwd: __dirname, logger: { info() {}, error() {} }, timeout: 2000,
    env: { ...process.env, SAI_TEST_MODE: mode }, ...overrides,
  });
}

test('启动协议拒绝外部地址、非法端口和缺失令牌', () => {
  assert.equal(parseStartupUrl('Sai Web: http://127.0.0.1:34567/?token=abc-_').port, '34567');
  for (const value of [
    'Sai Web: http://example.com:1234/?token=abc',
    'Sai Web: http://127.0.0.1:0/?token=abc',
    'Sai Web: http://127.0.0.1:99999/?token=abc',
    'Sai Web: http://127.0.0.1:34567/',
    'prefix Sai Web: http://127.0.0.1:34567/?token=abc',
  ]) assert.equal(parseStartupUrl(value), null);
});

test('启动等待完整输出与健康响应，重复停止后端口关闭', async () => {
  const backend = fixture();
  try {
    const url = await backend.start();
    assert.equal(url.searchParams.get('token'), 'test-secret_token');
    assert.equal(await checkHealth(url.origin), true);
    await Promise.all([backend.stop(), backend.stop()]);
    assert.equal(await checkHealth(url.origin), false);
    assert.notEqual(backend.child.exitCode, null);
  } finally {
    await backend.stop();
  }
});

test('不存在的程序和启动时退出都会返回错误', async () => {
  await assert.rejects(fixture('ready', { executable: '/missing/sai-test' }).start(), /ENOENT/);
  await assert.rejects(fixture('exit').start(), /17/);
});

test('启动超时和健康检查失败都会回收后端', async () => {
  for (const mode of ['hang', 'unhealthy']) {
    const backend = fixture(mode, { timeout: 400 });
    await assert.rejects(backend.start(), /未就绪/);
    assert.ok(backend.child.exitCode !== null || backend.child.signalCode !== null);
  }
});

test('启动过程中取消会停止子进程', async () => {
  const backend = fixture('hang');
  const starting = backend.start();
  const rejected = assert.rejects(starting, /取消|退出/);
  await backend.stop();
  await rejected;
});

test('就绪后的异常退出只通知一次', async () => {
  const failures = [];
  const backend = fixture('crash', { onExit: (error) => failures.push(error) });
  await backend.start();
  await backend.closed;
  assert.equal(failures.length, 1);
  assert.match(failures[0].message, /18/);
  await backend.stop();
});
