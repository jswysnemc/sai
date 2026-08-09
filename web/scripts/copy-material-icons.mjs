#!/usr/bin/env node
/**
 * 按映射清单裁剪复制 Material Icon Theme 图标。
 *
 * 正则扫描 src/shared/ui/material-icons.ts 中引用的图标名（源码即清单），
 * 只把用到的 SVG 从 node_modules 复制到 public/material-icons/，
 * 目录图标自动补 -open 变体；缺失图标名以非零码退出，防止映射漂移。
 */
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const mappingPath = join(root, "src/shared/ui/material-icons.ts");
const sourceDir = join(root, "node_modules/material-icon-theme/icons");
const targetDir = join(root, "public/material-icons");

// 1. 从映射源码提取全部图标名：对象映射值与扩展名数组的第二元素
const mapping = readFileSync(mappingPath, "utf8");
const names = new Set();
for (const match of mapping.matchAll(/:\s*"([a-z0-9_-]+)"/g)) {
  names.add(match[1]);
}
for (const match of mapping.matchAll(/,\s*"([a-z0-9_-]+)"\]/g)) {
  names.add(match[1]);
}
names.delete("file");
names.delete("directory");
// 2. 目录图标补 -open 变体；默认图标显式纳入
for (const name of [...names]) {
  if (name.startsWith("folder")) names.add(`${name}-open`);
}
names.add("file");
names.add("folder");
names.add("folder-open");

// 3. 只复制存在于图标包中的名字，缺失的映射名视为错误
if (!existsSync(sourceDir)) {
  console.error("material-icon-theme 未安装，跳过图标复制");
  process.exit(0);
}
rmSync(targetDir, { recursive: true, force: true });
mkdirSync(targetDir, { recursive: true });
const available = new Set(readdirSync(sourceDir));
const missing = [];
let copied = 0;
for (const name of names) {
  const file = `${name}.svg`;
  if (!available.has(file)) {
    // 提取阶段会混入映射键（扩展名、文件名），仅报告像图标名的缺失项
    if (!name.includes(".") && name !== "folder-open-open") missing.push(name);
    continue;
  }
  copyFileSync(join(sourceDir, file), join(targetDir, file));
  copied += 1;
}
if (missing.length > 0) {
  console.error(`映射引用了图标包中不存在的图标: ${missing.join(", ")}`);
  process.exit(1);
}
console.log(`已复制 ${copied} 个 Material 图标到 public/material-icons/`);
