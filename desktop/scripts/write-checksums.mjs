import { createReadStream } from 'node:fs';
import { mkdir, readdir, rename, rm, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const dist = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'dist');
const packages = path.join(dist, 'packages');
const installer = /\.(AppImage|tar\.gz|exe|dmg|zip|deb)$/;

/**
 * 【桌面构建】【产物收集】把安装包移出解压目录，避免和 linux-unpacked 等文件夹混放
 * @returns {Promise<string[]>} 已收集的安装包文件名
 */
async function collectInstallers() {
  await rm(packages, { recursive: true, force: true });
  await mkdir(packages, { recursive: true });
  const names = (await readdir(dist)).filter((name) => installer.test(name)).sort();
  for (const name of names) {
    await rename(path.join(dist, name), path.join(packages, name));
  }
  return (await readdir(packages)).filter((name) => installer.test(name)).sort();
}

const files = await collectInstallers();
if (!files.length) throw new Error('dist/packages 中没有可校验的安装包');
const checksums = [];
for (const name of files) {
  // 1. 【桌面构建】【校验文件】按安装包逐个写入 sha256，避免多平台清单互相覆盖
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(path.join(packages, name))) hash.update(chunk);
  const digest = hash.digest('hex');
  checksums.push(`${digest}  ${name}`);
  await writeFile(path.join(packages, `${name}.sha256`), `${digest}  ${name}\n`);
}
await writeFile(path.join(packages, 'SHA256SUMS'), `${checksums.join('\n')}\n`);
console.info(`【桌面构建】【校验完成】已收集 ${files.length} 个安装包`);
