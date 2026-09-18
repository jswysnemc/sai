import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";
import type { Session, WorkspaceSessions } from "../../api/contracts";
import { useSessionTree } from "./use-session-tree";

it("终端改变共享指针后，侧栏仍与 Web 消息区的当前会话一致", () => {
  const client = new QueryClient();
  const sessions: Session[] = ["A", "B"].map((id) => ({ id, title: id, active: id === "A", created_at: "now", updated_at: "now" }));
  client.setQueryData(["sessions"], sessions);
  client.setQueryData<WorkspaceSessions[]>(["session-tree"], [{
    workspace_id: "workspace", workspace_name: "Test", workspace_path: "/tmp/test", active: true,
    is_git_repository: false, sessions: sessions.map((session) => ({ ...session, active: session.id === "B" }))
  }]);
  let selected: string | undefined;
  /** 获取实际侧栏查询选择结果，返回空渲染内容。 */
  function Probe() {
    selected = useSessionTree().tree.data?.[0].sessions.find((session) => session.active)?.id;
    return null;
  }
  renderToStaticMarkup(<QueryClientProvider client={client}><Probe /></QueryClientProvider>);
  client.clear();
  expect(selected).toBe("A");
});
