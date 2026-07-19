export type GitPatchHunk = {
  id: string;
  path: string;
  patch: string;
};

/**
 * 将多文件 unified diff 拆成可独立交给 git apply 的 hunk patch。
 *
 * @param source 完整 Git unified diff
 * @returns 保留文件头的独立 hunk 列表
 */
export function splitGitPatchHunks(source: string): GitPatchHunk[] {
  const lines = source.replaceAll("\r\n", "\n").split("\n");
  const hunks: GitPatchHunk[] = [];
  let fileHeader: string[] = [];
  let currentHunk: string[] = [];
  let path = "";
  let fileIndex = -1;
  let hunkIndex = 0;

  /** 保存当前 hunk，并保留对应文件元信息。 */
  const flushHunk = () => {
    if (fileHeader.length === 0 || currentHunk.length === 0) return;
    hunks.push({
      id: `${fileIndex}:${hunkIndex}`,
      path,
      patch: [...fileHeader, ...currentHunk].join("\n").replace(/\n*$/, "\n")
    });
    hunkIndex += 1;
    currentHunk = [];
  };

  for (const line of lines) {
    // 1. 新文件头会结束上一文件的最后一个 hunk
    if (line.startsWith("diff --git ")) {
      flushHunk();
      fileIndex += 1;
      hunkIndex = 0;
      fileHeader = [line];
      currentHunk = [];
      path = parseDiffPath(line);
      continue;
    }
    if (fileHeader.length === 0) continue;

    // 2. 新 hunk 继承同一文件头，形成可独立应用的 patch
    if (line.startsWith("@@")) {
      flushHunk();
      currentHunk = [line];
      continue;
    }

    // 3. 首个 hunk 前的 index、rename、--- 和 +++ 均属于文件头
    if (currentHunk.length === 0) {
      fileHeader.push(line);
    } else {
      currentHunk.push(line);
    }
  }
  flushHunk();
  return hunks;
}

/**
 * 从 diff --git 文件头提取新路径。
 *
 * @param header Git 文件头
 * @returns 去除 b/ 前缀的路径
 */
function parseDiffPath(header: string): string {
  const marker = " b/";
  const index = header.lastIndexOf(marker);
  if (index < 0) return header;
  return header.slice(index + marker.length).replace(/^"|"$/g, "");
}
