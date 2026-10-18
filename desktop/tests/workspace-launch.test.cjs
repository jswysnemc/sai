const { test } = require('node:test');
const assert = require('node:assert/strict');
const { workspaceFromLaunch } = require('../electron/workspace-launch.cjs');

const context = {
  execPath: '/opt/sai-desktop/sai-desktop',
  appPath: '/opt/sai-desktop/resources/app',
  isDirectory: (candidate) => candidate === '/home/me/project' || candidate === '/opt/sai-desktop/resources/app',
};

test('目录参数优先于可执行文件和应用目录', () => {
  const argv = ['/opt/sai-desktop/sai-desktop', '/opt/sai-desktop/resources/app', '/home/me/project'];
  assert.equal(workspaceFromLaunch(argv, context), '/home/me/project');
});

test('没有目录参数时不打开工作区', () => {
  assert.equal(workspaceFromLaunch(['/opt/sai-desktop/sai-desktop', '--no-sandbox'], context), null);
});
