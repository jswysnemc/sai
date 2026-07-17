import { useEffect, useState } from "react";
import type { ThinkingLevel } from "../../api/contracts";

const GLOBAL_KEY = "sai.thinking-level";
const THINKING_LEVELS: ThinkingLevel[] = ["auto", "none", "low", "medium", "high", "xhigh", "max"];
const sessionKey = (sessionId?: string) => (sessionId ? `sai.thinking-level.${sessionId}` : GLOBAL_KEY);

/**
 * 管理思考等级和按会话隔离的浏览器本地偏好。
 */
export function useThinkingLevel(sessionId?: string) {
  const [thinkingLevel, setThinkingLevel] = useState<ThinkingLevel>(() => loadThinkingLevel(sessionId));

  useEffect(() => {
    setThinkingLevel(loadThinkingLevel(sessionId));
  }, [sessionId]);

  useEffect(() => {
    window.localStorage.setItem(sessionKey(sessionId), thinkingLevel);
  }, [thinkingLevel, sessionId]);

  return { thinkingLevel, setThinkingLevel };
}

function loadThinkingLevel(sessionId?: string): ThinkingLevel {
  const stored = (window.localStorage.getItem(sessionKey(sessionId)) ??
    (sessionId ? window.localStorage.getItem(GLOBAL_KEY) : null)) as ThinkingLevel | null;
  return stored && THINKING_LEVELS.includes(stored) ? stored : "auto";
}
