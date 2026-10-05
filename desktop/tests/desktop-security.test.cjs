const { test } = require('node:test');
const assert = require('node:assert/strict');
const { sanitizeAppearance } = require('../electron/window/window-controls.cjs');
const { allowedPageUrl } = require('../electron/browser-host/browser-tabs.cjs');
const { allowedOrigin } = require('../electron/browser-host/cdp-server.cjs');

test('标题栏只接收固定配色键，拒绝样式及 URL 注入', () => {
  assert.deepEqual(sanitizeAppearance({ paper: '#ffffff', ink: 'rgb(10, 20, 30)',
    surface: 'color-mix(in srgb, #fff 35%, #f5f5f5)', extra: 'red', line: 'red; display:none', muted: 'url(https://example.com)' }),
  { paper: '#ffffff', ink: 'rgb(10, 20, 30)', surface: 'color-mix(in srgb, #fff 35%, #f5f5f5)' });
});

test('浏览器页面不能导航到本地文件或特权协议', () => {
  for (const url of ['file:///etc/passwd', 'javascript:evil()', 'data:text/html,hello', 'devtools://devtools', 'invalid']) {
    assert.equal(allowedPageUrl(url), false);
  }
  for (const url of ['about:blank', 'http://127.0.0.1:3000', 'https://example.com']) assert.equal(allowedPageUrl(url), true);
});

test('浏览器协议拒绝第三方网页 Origin，只允许本机客户端和同源调试页', () => {
  assert.equal(allowedOrigin(undefined, 30303), true);
  assert.equal(allowedOrigin('http://127.0.0.1:30303', 30303), true);
  for (const origin of ['null', 'https://example.com', 'http://127.0.0.1:30304']) assert.equal(allowedOrigin(origin, 30303), false);
});
