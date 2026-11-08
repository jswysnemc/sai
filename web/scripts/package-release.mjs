import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * 【Web 分发】【发布打包】按后端版本生成静态资源包及摘要，供首次启动下载
 * @param {string} destination 发布产物目录
 * @returns {Promise<void>} 归档和校验文件写入完成
 */
async function packageRelease(destination) {
  const root = fileURLToPath(new URL('../../', import.meta.url));
  const dist = join(root, 'web/dist');
  const cargo = await readFile(join(root, 'Cargo.toml'), 'utf8');
  const version = cargo.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (!version) throw new Error('Cargo.toml 缺少版本号');
  const frontend = JSON.parse(await readFile(join(root, 'web/package.json'), 'utf8'));
  if (frontend.version !== version) throw new Error('前端与后端版本号不一致');
  if (process.env.RELEASE_TAG && process.env.RELEASE_TAG !== `v${version}`) {
    throw new Error('发布标签与后端版本号不一致');
  }
  await readFile(join(dist, 'index.html'));
  await readdir(join(dist, 'assets'));
  // 1. 【Web 分发】【版本清单】清单与后端版本匹配，资源目录内不包含发布包自身
  await writeFile(join(dist, 'sai-web.json'), `${JSON.stringify({ version })}\n`);
  await mkdir(destination, { recursive: true });
  const archive = join(destination, 'sai-web.tar.gz');
  const entries = (await readdir(dist)).sort();
  execFileSync('tar', ['-czf', archive, '-C', dist, ...entries], { stdio: 'inherit' });
  // 2. 【Web 分发】【下载校验】摘要使用固定文件名，下载端校验实际压缩包字节
  const digest = createHash('sha256').update(await readFile(archive)).digest('hex');
  await writeFile(`${archive}.sha256`, `${digest}  sai-web.tar.gz\n`);
  console.info(`【Web 分发】【发布完成】v${version} ${archive}`);
}

packageRelease(resolve(process.argv[2] || 'target/web-release')).catch((error) => {
  console.error(`【Web 分发】【发布失败】${error.message}`);
  process.exitCode = 1;
});
