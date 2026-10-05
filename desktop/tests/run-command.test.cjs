const test = require('node:test');
const assert = require('node:assert/strict');

// 【桌面构建】【命令执行】Windows 的 npm/pnpm 包装脚本必须经 shell 启动
test('Windows 包管理器命令经 shell 启动，其余平台与程序直接启动', async () => {
  const { needsShell } = await import('../scripts/run-command.mjs');
  for (const program of ['npm', 'pnpm', 'npx']) {
    assert.equal(needsShell(program, 'win32'), true, program);
    assert.equal(needsShell(program, 'linux'), false, program);
    assert.equal(needsShell(program, 'darwin'), false, program);
  }
  assert.equal(needsShell('cargo', 'win32'), false);
});
