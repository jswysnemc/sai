const { test } = require('node:test');
const assert = require('node:assert/strict');
const { controlsBackground } = require('../electron/window/caption-hit-test.cjs');

test('Windows 按钮视图必须不透明，主题纸色优先', () => {
  assert.equal(controlsBackground('win32', '#1c1c1c'), '#1c1c1c');
  assert.equal(controlsBackground('win32', ''), '#f7f7f7');
  assert.equal(controlsBackground('win32', 'transparent'), '#f7f7f7');
  assert.equal(controlsBackground('win32', '#00000000'), '#f7f7f7');
});

test('Linux 保持透明叠加，macOS 不创建按钮视图', () => {
  assert.equal(controlsBackground('linux', '#1c1c1c'), '#00000000');
  assert.equal(controlsBackground('darwin', '#1c1c1c'), null);
});
