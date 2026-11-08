import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

/**
 * 【发布】【版本检查】检查 CLI、Web、桌面及发布标签使用同一版本
 * @returns {Promise<void>} 版本一致时完成，不一致时抛出错误
 */
async function checkVersions() {
  const root = new URL('../../', import.meta.url);
  const manifest = await readFile(new URL('Cargo.toml', root), 'utf8');
  const version = manifest.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  assert.ok(version, 'Cargo.toml 缺少版本号');
  // 1. 【发布】【版本检查】同时检查包清单与锁文件中的根包版本
  for (const filename of ['web/package.json', 'web/package-lock.json', 'desktop/package.json']) {
    const metadata = JSON.parse(await readFile(new URL(filename, root), 'utf8'));
    assert.equal(metadata.version, version, `${filename} 与 CLI 版本不一致`);
    if (metadata.packages) {
      assert.equal(metadata.packages[''].version, version, `${filename} 根包版本不一致`);
    }
  }
  const lock = await readFile(new URL('Cargo.lock', root), 'utf8');
  const lockedVersion = lock.match(/\[\[package\]\]\s+name = "sai"\s+version = "([^"]+)"/)?.[1];
  assert.equal(lockedVersion, version, 'Cargo.lock 与 CLI 版本不一致');
  // 2. 【发布】【版本检查】标签发布必须与清单版本完全一致
  if (process.env.RELEASE_TAG) {
    assert.equal(process.env.RELEASE_TAG, `v${version}`, '发布标签与清单版本不一致');
  }
  console.info(`【发布】【版本检查】v${version}`);
}

await checkVersions();
