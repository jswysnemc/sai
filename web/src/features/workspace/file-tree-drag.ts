import type { DragEvent } from "react";

/**
 * 【文件树】【拖放】内部移动与外部文件落入工作区。
 */

const INTERNAL_TYPE = "application/x-sai-file-path";

/**
 * 写入内部拖拽路径。
 * @param event 拖拽事件
 * @param path 工作区相对路径
 * @returns 无
 */
export function setInternalDrag(event: DragEvent, path: string): void {
  event.dataTransfer.setData(INTERNAL_TYPE, path);
  event.dataTransfer.effectAllowed = "move";
}

/**
 * 读取内部拖拽路径。
 * @param event 拖放事件
 * @returns 相对路径；不是内部拖拽时返回空
 */
export function readInternalDrag(event: DragEvent): string {
  return event.dataTransfer.getData(INTERNAL_TYPE);
}

/**
 * 判断目标是否是源目录的自身或子路径。
 * @param source 被拖动的路径
 * @param target 落点目录
 * @returns 不能移动时为 true
 */
export function isNestedDrop(source: string, target: string): boolean {
  if (!source) return true;
  if (!target) return false;
  return source === target || target.startsWith(`${source}/`);
}

/**
 * 目录落点：目录用自身，文件用父目录。
 * @param path 行路径
 * @param directory 是否目录
 * @returns 目标目录，根为空字符串
 */
export function dropDirectory(path: string, directory: boolean): string {
  if (directory) return path;
  const index = path.lastIndexOf("/");
  return index < 0 ? "" : path.slice(0, index);
}

/**
 * 读取外部拖入的文件。
 * @param event 拖放事件
 * @returns 浏览器 File 列表
 */
export function droppedFiles(event: DragEvent): File[] {
  return Array.from(event.dataTransfer.files ?? []);
}
