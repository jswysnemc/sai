import type { FileContent } from "../../api/contracts";

export type EditorDocumentState = {
  path: string | null;
  content: string;
  baseline: FileContent | null;
  latestRemote: FileContent | null;
  externalChange: boolean;
};

/**
 * 创建指定路径的空编辑器文档状态。
 *
 * @param path 当前文件路径
 * @returns 尚未载入远端内容的文档状态
 */
export function createEditorDocumentState(path: string | null): EditorDocumentState {
  return { path, content: "", baseline: null, latestRemote: null, externalChange: false };
}

/**
 * 将查询得到的文件快照应用到编辑器状态。
 *
 * @param state 当前编辑器状态
 * @param remote 最新远端文件快照
 * @returns 保留脏草稿和固定保存基线的新状态
 */
export function applyRemoteFile(
  state: EditorDocumentState,
  remote: FileContent
): EditorDocumentState {
  if (!state.baseline) return loadedDocument(state.path, remote);
  const dirty = state.content !== state.baseline.content;
  if (!dirty) return loadedDocument(state.path, remote);
  return {
    ...state,
    latestRemote: remote,
    externalChange: remote.version !== state.baseline.version
  };
}

/**
 * 更新编辑器草稿内容，并重新计算外部变化状态。
 *
 * @param state 当前编辑器状态
 * @param content Monaco 返回的最新草稿
 * @returns 更新后的编辑器状态
 */
export function updateDocumentContent(
  state: EditorDocumentState,
  content: string
): EditorDocumentState {
  if (!state.baseline) return { ...state, content };
  if (
    content === state.baseline.content &&
    state.latestRemote &&
    state.latestRemote.version !== state.baseline.version
  ) {
    return loadedDocument(state.path, state.latestRemote);
  }
  return {
    ...state,
    content,
    externalChange: Boolean(
      state.latestRemote &&
      state.latestRemote.version !== state.baseline.version &&
      content !== state.baseline.content
    )
  };
}

/**
 * 明确丢弃草稿并载入最近一次远端快照。
 *
 * @param state 当前编辑器状态
 * @returns 已重载的编辑器状态；没有远端快照时保持不变
 */
export function reloadRemoteFile(state: EditorDocumentState): EditorDocumentState {
  return state.latestRemote ? loadedDocument(state.path, state.latestRemote) : state;
}

/**
 * 将保存接口返回的文件设为新基线。
 *
 * @param state 当前编辑器状态
 * @param saved 保存成功后的文件快照
 * @returns 保存后的干净文档状态
 */
export function acceptSavedFile(
  state: EditorDocumentState,
  saved: FileContent
): EditorDocumentState {
  return loadedDocument(state.path, saved);
}

/**
 * 判断当前草稿是否可以安全保存。
 *
 * @param state 当前编辑器状态
 * @returns 已载入、内容变化且没有外部冲突时返回 true
 */
export function canSaveDocument(state: EditorDocumentState): boolean {
  return Boolean(
    state.baseline &&
    state.content !== state.baseline.content &&
    !state.externalChange
  );
}

/**
 * 用一个远端快照构造干净文档状态。
 *
 * @param path 当前文件路径
 * @param remote 远端文件快照
 * @returns 已载入的干净文档状态
 */
function loadedDocument(path: string | null, remote: FileContent): EditorDocumentState {
  return {
    path,
    content: remote.content,
    baseline: remote,
    latestRemote: remote,
    externalChange: false
  };
}
