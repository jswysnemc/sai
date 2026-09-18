import type { WebEvent } from "../../api/contracts";

type SessionScope = { workspaceId?: string; sessionId?: string };

/**
 * 【会话同步】【异步归属】为每次会话选择保留独立标识，隔离旧请求和旧事件流。
 * @returns 选择会话、检查请求归属和事件归属的方法
 */
export function createSessionRunScope() {
  let current: SessionScope = {};
  return {
    /**
     * 选择当前工作区与会话；离开后再次进入同一会话时生成新标识。
     * @param workspaceId 工作区标识
     * @param sessionId 会话标识
     * @returns 当前选择标识
     */
    select(workspaceId?: string, sessionId?: string): SessionScope {
      if (current.workspaceId !== workspaceId || current.sessionId !== sessionId) current = { workspaceId, sessionId };
      return current;
    },
    /** 检查请求持有的选择标识，参数为请求标识，返回是否仍属于当前选择。 */
    isCurrent(scope: SessionScope): boolean { return scope === current; },
    /**
     * 校验事件来源与递增序号。
     * @param scope 事件流建立时的选择标识
     * @param event 接收到的事件
     * @param lastSequence 已接收的最后序号
     * @returns 是否允许事件修改当前运行状态
     */
    accepts(scope: SessionScope, event: WebEvent, lastSequence: number): boolean {
      return scope === current && event.session_id === scope.sessionId && event.workspace_id === scope.workspaceId
        && (event.sequence === 0 || event.sequence > lastSequence);
    }
  };
}
