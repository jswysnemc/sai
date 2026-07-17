import { createContext, useContext, type ReactNode } from "react";
import { useChatAgent } from "./use-chat-agent";

type ChatAgentContextValue = ReturnType<typeof useChatAgent>;

const ChatAgentContext = createContext<ChatAgentContextValue | null>(null);

/**
 * 提供主界面 Agent 列表、当前选择和本地偏好。
 *
 * @param props 子组件内容
 * @returns Agent 状态上下文提供者
 */
export function ChatAgentProvider({ children }: { children: ReactNode }) {
  const agent = useChatAgent();
  return <ChatAgentContext.Provider value={agent}>{children}</ChatAgentContext.Provider>;
}

/**
 * 读取主界面共享的 Agent 状态。
 *
 * @returns Agent 列表、当前选择和更新方法
 */
export function useChatAgentContext(): ChatAgentContextValue {
  const context = useContext(ChatAgentContext);
  if (!context) throw new Error("useChatAgentContext 必须在 ChatAgentProvider 内使用");
  return context;
}
