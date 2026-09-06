const unsavedEditors = new Map<symbol, string>();

/**
 * 登记一个存在未保存修改的编辑器，同一文件的多个编辑器分别管理。
 * @param path 文件路径
 * @returns 保存、还原或卸载编辑器时调用的清理方法
 */
export function registerUnsavedEditor(path: string): () => void {
  const editorId = Symbol(path);
  unsavedEditors.set(editorId, path);
  return () => { unsavedEditors.delete(editorId); };
}

/**
 * 获取当前编辑器中尚未保存的文件路径，不读取或保存文件内容。
 * @returns 去重后的文件路径列表
 */
export function getUnsavedEditorPaths(): string[] {
  return [...new Set(unsavedEditors.values())];
}
