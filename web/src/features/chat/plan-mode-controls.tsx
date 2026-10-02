import { useRef, useState } from "react";
import type { RunMode } from "../../api/contracts";
import { apiRequest } from "../../api/api-request";
import { Button } from "../../shared/ui/button/button";
import { NotepadText } from "../../shared/ui/icons";
import { useI18n } from "../i18n/use-i18n";

/** 【计划模式】【命令解析】参数为输入文本，返回 /plan 后的任务；非命令返回 null。 */
export function parsePlanCommand(value: string): string | null {
  const match = value.trim().match(/^\/plan(?:\s+([\s\S]*))?$/i);
  return match ? (match[1] ?? "").trim() : null;
}

/** 【计划模式】【独立入口】参数为会话状态及更新回调，返回规划入口、权限恢复值和错误。 */
export function usePlanModeControls(sessionId: string | undefined, mode: RunMode, onModeChange: (mode: RunMode) => void) {
  const sessionRef = useRef(sessionId);
  sessionRef.current = sessionId;
  const previous = useRef<{ sessionId?: string; mode: RunMode }>({ sessionId, mode: mode === "plan" ? "audited" : mode });
  if (previous.current.sessionId !== sessionId || mode !== "plan") {
    previous.current = { sessionId, mode: mode === "plan" ? "audited" : mode };
  }
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [error, setError] = useState("");

  /** 【计划模式】【进入规划】无参数，保存返回权限后切换模式，返回是否成功。 */
  async function enter(): Promise<boolean> {
    if (!sessionId || busyRef.current) return false;
    if (mode === "plan") return true;
    busyRef.current = true; setBusy(true); setError("");
    try {
      await apiRequest(`/api/sessions/${encodeURIComponent(sessionId)}/plan`, {
        method: "POST", body: JSON.stringify({ execution_mode: mode })
      });
      if (sessionRef.current !== sessionId) return false;
      onModeChange("plan");
      return true;
    } catch (failure) {
      if (sessionRef.current === sessionId) setError(failure instanceof Error ? failure.message : String(failure));
      return false;
    } finally { busyRef.current = false; setBusy(false); }
  }
  return { enter, busy, error, normalMode: previous.current.mode };
}

/** 【计划模式】【状态控件】参数为当前模式及切换回调，返回与权限列表分离的规划按钮。 */
export function PlanModeControls({ mode, disabled, onEnter, onLeave }: {
  mode: RunMode; disabled: boolean; onEnter: () => void; onLeave: () => void;
}) {
  const { t } = useI18n();
  return <Button size="small" variant={mode === "plan" ? "secondary" : "ghost"} disabled={disabled}
    aria-pressed={mode === "plan"} title={mode === "plan" ? t("Leave planning without executing", "退出规划，不执行计划") : t("Plan first · /plan", "先规划 · /plan")}
    onClick={mode === "plan" ? onLeave : onEnter}>
    <NotepadText size={14} />{mode === "plan" ? t("Plan · Exit", "计划中 · 退出") : t("Plan", "计划")}
  </Button>;
}
