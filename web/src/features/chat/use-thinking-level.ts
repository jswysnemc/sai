import { useEffect, useState } from "react";
import type { ThinkingLevel } from "../../api/contracts";
import {
  readStoredThinkingLevel,
  writeStoredThinkingLevel
} from "./session-preference-storage";

type StoredThinkingPreference = {
  sessionId?: string;
  level: ThinkingLevel;
};

/**
 * 【会话】【思考偏好】管理思考等级和按会话隔离的浏览器本地偏好。
 *
 * @param sessionId 当前会话 ID
 * @returns 当前思考等级和更新方法
 */
export function useThinkingLevel(sessionId?: string) {
  const [storedPreference, setStoredPreference] = useState<StoredThinkingPreference>(() => ({
    sessionId,
    level: readStoredThinkingLevel(sessionId)
  }));
  const thinkingLevel = storedPreference.sessionId === sessionId
    ? storedPreference.level
    : readStoredThinkingLevel(sessionId);

  useEffect(() => {
    setStoredPreference((current) => current.sessionId === sessionId
      ? current
      : { sessionId, level: readStoredThinkingLevel(sessionId) });
  }, [sessionId]);

  useEffect(() => {
    writeStoredThinkingLevel(sessionId, thinkingLevel);
  }, [thinkingLevel, sessionId]);

  /**
   * 【会话】【思考偏好】更新当前会话的思考等级。
   *
   * @param level 新思考等级
   * @returns 无返回值
   */
  const setThinkingLevel = (level: ThinkingLevel) => {
    setStoredPreference({ sessionId, level });
  };

  return { thinkingLevel, setThinkingLevel };
}
