import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SessionWorkspaceIcon } from "./session-workspace-icon";

/**
 * 渲染工作区图标的静态标记。
 *
 * @param isGitRepository 工作区是否属于 Git 仓库
 * @returns 可用于图标类型断言的 SVG 标记
 */
function renderIcon(isGitRepository: boolean): string {
  return renderToStaticMarkup(<SessionWorkspaceIcon isGitRepository={isGitRepository} size={14} />);
}

describe("SessionWorkspaceIcon", () => {
  it("distinguishes Git repositories from ordinary directories", () => {
    expect(renderIcon(true)).toContain("lucide-folder-git2");
    expect(renderIcon(false)).toContain("lucide-folder");
    expect(renderIcon(false)).not.toContain("lucide-folder-git2");
  });
});
