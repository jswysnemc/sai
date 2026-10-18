const path = require('node:path');
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { workspaceFromLaunch } = require('../electron/workspace-launch.cjs');

const execPath = path.resolve('/opt/sai-desktop/sai-desktop');
const appPath = path.resolve('/opt/sai-desktop/resources/app');
const project = path.resolve('/home/me/project');
const context = {
  execPath,
  appPath,
  isDirectory: (candidate) => candidate === project || candidate === appPath,
};

test('目录参数优先于可执行文件和应用目录', () => {
  const argv = [execPath, appPath, project];
  assert.equal(workspaceFromLaunch(argv, context), project);
});

test('没有目录参数时不打开工作区', () => {
  assert.equal(workspaceFromLaunch([execPath, '--no-sandbox'], context), null);
});
