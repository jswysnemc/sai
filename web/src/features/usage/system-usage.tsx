import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Cpu, Gauge, X } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { api } from "../../api/client";
import type { RunMode, RunModelSelection } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { Modal } from "../../shared/ui/dialog/modal";
import { useAnchoredPopover } from "../../shared/ui/popover/use-anchored-popover";
import { useI18n } from "../i18n/use-i18n";
import { COMPACTION_POLICY_FALLBACK, CompactionPolicyPanel } from "./compaction-policy/compaction-policy-panel";
import { ContextUsagePanel } from "./context-usage-panel";
import { ProcessUsagePanel } from "./process-usage-panel";
import { formatTokenCount } from "./token-format";
import "./system-usage.css";

export { formatContextCacheDetail } from "./usage-format";

/**
 * 分别提供上下文用量和进程资源入口，共享同一份用量查询。
 * @param props 当前模型、运行模式、压缩回调和禁用状态
 * @returns 两个紧凑入口及各自的详情弹层
 */
export function SystemUsage({ selection, mode, agentId, onCompact, compactDisabled }: {
  selection: RunModelSelection | null;
  mode: RunMode;
  agentId?: string | null;
  onCompact: () => Promise<void>;
  compactDisabled: boolean;
}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [open, setOpen] = useState<"context" | "process" | null>(null);
  const [policyOpen, setPolicyOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const contextRef = useRef<HTMLButtonElement>(null);
  const processRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const titleId = useId();
  const usage = useQuery({
    queryKey: ["system-usage", selection?.providerId, selection?.model, mode, agentId ?? ""],
    queryFn: () => api.system.usage(selection, mode, agentId),
    refetchInterval: open || policyOpen ? 2_000 : 5_000
  });
  const compact = useMutation({
    mutationFn: onCompact,
    onSettled: () => { void queryClient.invalidateQueries({ queryKey: ["system-usage"] }); }
  });
  const activeTrigger = open === "process" ? processRef : contextRef;
  const popoverStyle = useAnchoredPopover({
    open: open !== null, anchorRef: activeTrigger,
    preferredWidth: open === "process" ? 300 : 360, minimumWidth: 280, align: "right", maxHeight: 430,
    preferredHeight: open === "process" ? 250 : 340
  });
  const contextPercent = Math.round(Math.min(1, Math.max(0, usage.data?.session.context_token_ratio ?? 0)) * 100);

  useEffect(() => {
    if (!open) return;
    /** 关闭外部点击的弹层，保留点击目标的焦点。@param event 指针事件 @returns 无 */
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!rootRef.current?.contains(target) && !popoverRef.current?.contains(target)) setOpen(null);
    };
    /** 按 Escape 关闭并返回入口。@param event 键盘事件 @returns 无 */
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      setOpen(null);
      activeTrigger.current?.focus();
    };
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [activeTrigger, open]);

  return (
    <div className="system-usage" ref={rootRef}>
      <Button ref={contextRef} variant="ghost" size="small" className="system-usage-trigger" onClick={() => setOpen(open === "context" ? null : "context")} aria-expanded={open === "context"} aria-label={t("View context usage", "查看上下文用量")}>
        <span className="usage-ring" style={{ background: `conic-gradient(var(--signal) ${contextPercent}%, var(--line) 0)` }}><Gauge size={10} /></span>
        <span className="hidden sm:inline-flex"><strong>{usage.data ? formatTokenCount(usage.data.session.context_prompt_tokens) : "--"}</strong><small>{contextPercent}%</small></span>
      </Button>
      <Button ref={processRef} variant="ghost" size="icon" className="system-process-trigger" onClick={() => setOpen(open === "process" ? null : "process")} aria-expanded={open === "process"} aria-label={t("View process resources", "查看进程资源")}><Cpu size={14} /></Button>
      {open && createPortal(
        <div ref={popoverRef} className="system-usage-popover" style={popoverStyle} role="dialog" aria-labelledby={titleId}>
          <header>
            <strong id={titleId}>{open === "context" ? t("Context usage", "上下文用量") : t("Process resources", "进程资源")}</strong>
            <Button variant="ghost" size="icon" onClick={() => { setOpen(null); activeTrigger.current?.focus(); }} aria-label={t("Close", "关闭")}><X size={14} /></Button>
          </header>
          {usage.isLoading && <p className="usage-loading">{t("Loading usage", "正在读取用量")}</p>}
          {usage.error && <p className="usage-error" role="alert">{usage.error.message}</p>}
          {usage.data && (open === "context"
            ? <ContextUsagePanel usage={usage.data} compactPending={compact.isPending} compactDisabled={compactDisabled} compactError={compact.error?.message} onCompact={() => compact.mutate()} onConfigure={() => { setOpen(null); setPolicyOpen(true); }} />
            : <ProcessUsagePanel usage={usage.data} />)}
        </div>, document.body
      )}
      <Modal open={policyOpen} title={t("Context compaction", "上下文压缩设置")} size="small" onClose={() => { setPolicyOpen(false); contextRef.current?.focus(); }}>
        {usage.data && <CompactionPolicyPanel
          sessionId={usage.data.session.id}
          ratio={usage.data.session.compaction_ratio ?? COMPACTION_POLICY_FALLBACK.ratio}
          reserve={usage.data.session.compaction_reserve_tokens ?? COMPACTION_POLICY_FALLBACK.reserve}
          windowTokens={usage.data.session.context_window_tokens}
          usedTokens={usage.data.session.context_prompt_tokens}
          overridden={Boolean(usage.data.session.compaction_policy_override)}
          onSaved={() => { void queryClient.invalidateQueries({ queryKey: ["system-usage"] }); }}
        />}
      </Modal>
    </div>
  );
}
