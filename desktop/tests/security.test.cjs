const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { redact, createLogger } = require('../electron/logging.cjs');
const { isInternal, isExternal } = require('../electron/navigation.cjs');
const { readPort, savePort } = require('../electron/runtime-state.cjs');

test('外部导航拒绝脚本、本地文件、自定义协议和含凭据的地址', () => {
  const origin = 'http://127.0.0.1:33333';
  assert.equal(isInternal(`${origin}/sessions`, origin), true);
  assert.equal(isInternal('http://127.0.0.1:33334', origin), false);
  assert.equal(isInternal('http://127.0.0.1.evil.test:33333', origin), false);
  for (const value of ['javascript:alert(1)', 'file:///etc/passwd', 'data:text/html,test',
    'sai://test', 'https://user:password@example.com', 'not a URL']) {
    assert.equal(isExternal(value), false);
    assert.equal(isInternal(value, origin), false);
  }
  assert.equal(isExternal('https://example.com/docs'), true);
  assert.equal(isExternal('mailto:test@example.com'), true);
});

test('日志不记录启动令牌、认证头和会话 Cookie', () => {
  const result = redact('http://127.0.0.1:33333/?token=secret Bearer another sai_web_session=cookie;');
  for (const secret of ['secret', 'another', '=cookie']) assert.ok(!result.includes(secret));
});

test('日志轮换以及损坏的运行状态不会阻止恢复', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'sai-desktop-unit-'));
  try {
    const logger = createLogger(directory);
    fs.writeFileSync(logger.file, 'x'.repeat(2 * 1024 * 1024 + 1));
    logger.info('【桌面测试】【脱敏】http://127.0.0.1:33333/?token=secret');
    assert.ok(fs.existsSync(`${logger.file}.1`));
    assert.ok(!fs.readFileSync(logger.file, 'utf8').includes('secret'));
    const state = path.join(directory, 'runtime.json');
    assert.equal(readPort(state), 0);
    savePort(state, 33333);
    assert.equal(readPort(state), 33333);
    for (const value of ['broken', '{"port":1}', '{"port":"33333"}', '{"port":99999}']) {
      fs.writeFileSync(state, value);
      assert.equal(readPort(state), 0);
    }
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});
