import { Folder, FolderGit2 } from "lucide-react";

type SessionWorkspaceIconProps = {
  isGitRepository: boolean;
  size?: number;
};

/**
 * 按工作区类型渲染 Git 文件夹或普通文件夹图标。
 *
 * @param props Git 仓库标识和图标尺寸
 * @returns 工作区类型图标
 */
export function SessionWorkspaceIcon({ isGitRepository, size = 14 }: SessionWorkspaceIconProps) {
  return isGitRepository ? <FolderGit2 size={size} /> : <Folder size={size} />;
}
