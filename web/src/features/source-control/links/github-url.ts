/**
 * 把 Git 远端地址换算成 GitHub 网页链接。
 *
 * 远端地址存在 SSH（git@github.com:owner/repo.git）与 HTTPS 两种形态，
 * 这里统一规整成 https://github.com/owner/repo，再拼接提交或文件路径。
 * 非 GitHub 远端一律返回空串，由调用方据此隐藏入口。
 */

/**
 * 将任意形态的 GitHub 远端地址规整为仓库主页链接。
 *
 * @param remoteUrl 远端地址，支持 git@ 与 https:// 两种形态
 * @returns 形如 https://github.com/owner/repo 的链接，非 GitHub 远端返回空串
 */
export function normalizeGitHubRepositoryUrl(remoteUrl: string): string {
  const value = remoteUrl.trim();
  if (!value) return "";

  // 1. SSH 形态无法交给 URL 解析，用正则单独取出 owner 与 repo
  const sshMatch = /^git@github\.com:([^/\s]+)\/(.+?)(?:\.git)?$/i.exec(value);
  if (sshMatch?.[1] && sshMatch[2]) {
    return `https://github.com/${sshMatch[1]}/${sshMatch[2].replace(/\.git$/i, "")}`;
  }

  // 2. 其余形态按 URL 解析，仅接受 github.com 主机
  try {
    const url = new URL(value);
    if (!["github.com", "www.github.com"].includes(url.hostname.toLowerCase())) return "";
    const parts = url.pathname
      .replace(/^\/+|\/+$/g, "")
      .split("/")
      .filter(Boolean);
    const owner = parts[0];
    const repo = parts[1]?.replace(/\.git$/i, "");
    if (!owner || !repo) return "";
    return `https://github.com/${owner}/${repo}`;
  } catch {
    return "";
  }
}

/**
 * 拼接提交在 GitHub 上的链接。
 *
 * @param remoteUrl 远端地址
 * @param sha 提交完整或短哈希
 * @returns 提交页链接，远端非 GitHub 或缺少哈希时返回空串
 */
export function gitHubCommitUrl(remoteUrl: string, sha: string): string {
  const repoUrl = normalizeGitHubRepositoryUrl(remoteUrl);
  const commitSha = sha.trim();
  return repoUrl && commitSha ? `${repoUrl}/commit/${commitSha}` : "";
}

/**
 * 对文件路径逐段做 URL 编码。
 *
 * @param path 仓库内相对路径，允许包含 Windows 反斜杠
 * @returns 以斜杠连接的编码路径
 */
export function encodeGitHubPath(path: string): string {
  return path
    .replace(/\\/g, "/")
    .split("/")
    .filter(Boolean)
    .map((part) => encodeURIComponent(part))
    .join("/");
}

/**
 * 拼接某次提交下单个文件在 GitHub 上的链接。
 *
 * @param remoteUrl 远端地址
 * @param commitSha 提交哈希
 * @param path 文件相对路径
 * @param status 文件在该提交中的状态码，删除态无对应页面
 * @returns 文件页链接，删除态或远端非 GitHub 时返回空串
 */
export function gitHubFileUrl(
  remoteUrl: string,
  commitSha: string,
  path: string,
  status: string
): string {
  // 1. 已删除文件在该提交下没有 blob 页面
  if (status.charAt(0).toUpperCase() === "D") return "";

  const repoUrl = normalizeGitHubRepositoryUrl(remoteUrl);
  const sha = commitSha.trim();
  const encodedPath = encodeGitHubPath(path);
  return repoUrl && sha && encodedPath ? `${repoUrl}/blob/${sha}/${encodedPath}` : "";
}
