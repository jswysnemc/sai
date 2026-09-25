import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import type { Session, WorkspaceSessions } from "../../api/contracts";
import { WorkspaceListView } from "./workspace-list-view";

/**
 * 构造一条会话。
 *
 * @param title 标题
 * @returns 两个工作区可以共用的 default 会话
 */
function session(title: string): Session {
  return { id: "default", title, active: title === "one", created_at: "2026-09-24T00:00:00Z", updated_at: "2026-09-24T00:00:00Z" };
}

/**
 * 构造工作区。
 *
 * @param id 工作区 ID
 * @param name 名称
 * @param title 唯一会话标题
 * @returns 带一条 default 会话的工作区
 */
function workspace(id: string, name: string, title: string): WorkspaceSessions {
  return {
    workspace_id: id,
    workspace_name: name,
    workspace_path: `/tmp/${id}`,
    last_opened_at: "2026-09-24T00:00:00Z",
    active: id === "a",
    is_git_repository: false,
    sessions: [session(title)]
  };
}

it("展开后同时渲染不同工作区里同名 default 会话", () => {
  const html = renderToStaticMarkup(
    <WorkspaceListView
      workspaces={[workspace("a", "Alpha", "one"), workspace("b", "Beta", "two")]}
      runningSessions={new Set()}
      menuRef={{ current: null }}
      menu={null}
      onToggleMenu={() => undefined}
      onOpenSession={() => undefined}
      expandedWorkspaceIds={new Set(["a", "b"])}
      onToggleExpanded={() => undefined}
      onCreateSession={() => undefined}
      createPending={false}
      now={Date.parse("2026-09-24T00:00:00Z")}
      onCloseWorkspace={() => undefined}
      preserveOrder
      allowClose
    />
  );
  expect(html).toContain("one");
  expect(html).toContain("two");
  expect(html).toContain("Alpha");
  expect(html).toContain("Beta");
  expect(html.match(/workspace-session-item/g)?.length).toBe(2);
  expect(html).not.toContain("workspace-time");
  expect(html.match(/workspace-session-time/g)?.length).toBe(2);
});
