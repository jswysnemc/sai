import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Pause, Play, Target, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../api/client";
import type { Goal, GoalStatus } from "../../api/goal-contracts";
import { toDisplayError } from "../../api/api-error";
import { Button } from "../../shared/ui/button/button";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { Modal } from "../../shared/ui/dialog/modal";
import { TextArea } from "../../shared/ui/form/text-area";
import { useI18n } from "../i18n/use-i18n";
import "./goal-control.css";

type GoalControlProps = {
  sessionId?: string;
  running: boolean;
  onContinue: () => Promise<void>;
};

/**
 * 渲染会话 Goal 的紧凑状态入口与管理弹层。
 *
 * @param props 会话标识、运行状态和续轮回调
 * @returns Goal 控件
 */
export function GoalControl({ sessionId, running, onContinue }: GoalControlProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const [objective, setObjective] = useState("");
  const [tokenBudget, setTokenBudget] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const queryKey = ["goal", sessionId] as const;
  const goalQuery = useQuery({
    queryKey,
    queryFn: () => api.goals.read(sessionId!),
    enabled: Boolean(sessionId),
    refetchInterval: (query) => query.state.data?.goal?.status === "active" ? 2_000 : false
  });
  const goal = goalQuery.data?.goal ?? null;

  useEffect(() => {
    if (!open) return;
    setObjective(goal?.objective ?? "");
    setTokenBudget(goal?.token_budget ? String(goal.token_budget) : "");
  }, [goal?.id, open]);

  /** 打开目标弹层并清理旧错误。 */
  const openGoal = () => {
    setError(null);
    setOpen(true);
  };

  /**
   * 更新查询缓存中的 Goal。
   *
   * @param next 新 Goal
   * @returns 无返回值
   */
  const cacheGoal = (next: Goal | null) => {
    queryClient.setQueryData(queryKey, { goal: next });
  };

  /**
   * 创建或替换目标并启动自动续轮。
   *
   * @returns 保存完成后的 Promise
   */
  const save = async () => {
    if (!sessionId || !objective.trim()) return;
    const parsedBudget = tokenBudget.trim() ? Number(tokenBudget) : undefined;
    if (parsedBudget !== undefined && (!Number.isSafeInteger(parsedBudget) || parsedBudget <= 0)) {
      setError(new Error(t("Token budget must be a positive integer", "Token 预算必须为正整数")));
      return;
    }
    setBusy(true);
    setError(null);
    try {
      // 1. 新目标直接创建，已有目标原位更新并保留累计用量
      const response = goal
        ? await api.goals.update(sessionId, {
            objective: objective.trim(),
            token_budget: parsedBudget ?? null,
            status: "active"
          })
        : await api.goals.set(sessionId, objective, parsedBudget);
      cacheGoal(response.goal ?? null);
      // 2. 当前会话空闲时启动持续执行
      if (!running) await onContinue();
      setOpen(false);
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to save goal", "保存目标失败"));
    } finally {
      setBusy(false);
    }
  };

  /**
   * 修改目标状态，并按需恢复自动续轮。
   *
   * @param status 新目标状态
   * @returns 状态更新完成后的 Promise
   */
  const updateStatus = async (status: GoalStatus) => {
    if (!sessionId) return;
    setBusy(true);
    setError(null);
    try {
      const response = await api.goals.update(sessionId, { status });
      cacheGoal(response.goal ?? null);
      if (status === "active" && !running) await onContinue();
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to update goal", "更新目标失败"));
      setOpen(true);
    } finally {
      setBusy(false);
    }
  };

  /**
   * 启动空闲目标续轮并展示失败原因。
   *
   * @returns 启动完成后的 Promise
   */
  const continueGoal = async () => {
    setBusy(true);
    setError(null);
    try {
      await onContinue();
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to continue goal", "继续目标失败"));
      setOpen(true);
    } finally {
      setBusy(false);
    }
  };

  /**
   * 确认并清除当前目标。
   *
   * @returns 清除完成后的 Promise
   */
  const clear = async () => {
    if (!sessionId || !goal) return;
    const accepted = await confirm({
      title: t("Clear goal?", "清除目标？"),
      description: goal.objective,
      confirmLabel: t("Clear", "清除"),
      danger: true
    });
    if (!accepted) return;
    setBusy(true);
    setError(null);
    try {
      await api.goals.clear(sessionId);
      cacheGoal(null);
      setOpen(false);
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to clear goal", "清除目标失败"));
    } finally {
      setBusy(false);
    }
  };

  const statusLabel = goal ? goalStatusLabel(goal.status, t) : t("Goal", "目标");
  return (
    <>
      <div className={`goal-control${goal ? ` status-${goal.status}` : ""}`}>
        <Button
          className="composer-rail-button goal-control-trigger"
          onClick={openGoal}
          disabled={!sessionId}
          title={goal?.objective ?? t("Set goal", "设置目标")}
          aria-label={goal ? t("Open goal", "打开目标") : t("Set goal", "设置目标")}
        >
          <Target size={14} />
        </Button>
        {goal?.status === "active" && !running && (
          <Button className="composer-rail-button goal-control-action" onClick={() => void continueGoal()} disabled={busy} title={t("Continue goal", "继续目标")} aria-label={t("Continue goal", "继续目标")}>
            <Play size={13} />
          </Button>
        )}
        {goal?.status === "active" && (
          <Button className="composer-rail-button goal-control-action" onClick={() => void updateStatus("paused")} disabled={busy} title={t("Pause goal", "暂停目标")} aria-label={t("Pause goal", "暂停目标")}>
            <Pause size={13} />
          </Button>
        )}
        {goal && ["paused", "blocked", "usage_limited"].includes(goal.status) && (
          <Button className="composer-rail-button goal-control-action" onClick={() => void updateStatus("active")} disabled={busy || running} title={t("Resume goal", "恢复目标")} aria-label={t("Resume goal", "恢复目标")}>
            <Play size={13} />
          </Button>
        )}
      </div>
      <Modal
        open={open}
        title={goal ? t("Session goal", "会话目标") : t("Create goal", "创建目标")}
        description={goal ? statusLabel : undefined}
        size="small"
        onClose={() => setOpen(false)}
        footer={(
          <>
            {goal && <Button variant="danger" onClick={() => void clear()} disabled={busy}><Trash2 size={14} />{t("Clear", "清除")}</Button>}
            <Button onClick={() => setOpen(false)}>{t("Cancel", "取消")}</Button>
            <Button variant="primary" onClick={() => void save()} disabled={busy || !objective.trim()}>{goal ? t("Save and continue", "保存并继续") : t("Start", "开始")}</Button>
          </>
        )}
      >
        <div className="goal-control-form">
          <label>
            <span>{t("Objective", "目标内容")}</span>
            <TextArea value={objective} maxLength={32_000} onChange={(event) => setObjective(event.target.value)} />
          </label>
          <label>
            <span>{t("Token budget", "Token 预算")}</span>
            <input type="number" min="1" step="1" value={tokenBudget} onChange={(event) => setTokenBudget(event.target.value)} placeholder={t("Unlimited", "不限")} />
          </label>
          {goal && <GoalUsage goal={goal} />}
          {(error || goalQuery.error) && <div className="goal-control-error">{error?.message ?? goalQuery.error?.message}</div>}
        </div>
      </Modal>
    </>
  );
}

/**
 * 渲染目标用量摘要。
 *
 * @param props 当前目标
 * @returns 用量文本
 */
function GoalUsage({ goal }: { goal: Goal }) {
  const { t } = useI18n();
  return (
    <div className="goal-control-usage">
      <span>{t("Tokens", "Token")} {goal.tokens_used.toLocaleString()} / {goal.token_budget?.toLocaleString() ?? t("Unlimited", "不限")}</span>
      <span>{t("Time", "时间")} {formatDuration(goal.time_used_seconds)}</span>
    </div>
  );
}

/**
 * 返回本地化目标状态。
 *
 * @param status 目标状态
 * @param t 双语文本选择函数
 * @returns 状态文本
 */
function goalStatusLabel(status: GoalStatus, t: (english: string, chinese: string) => string): string {
  const labels: Record<GoalStatus, [string, string]> = {
    active: ["Active", "进行中"],
    paused: ["Paused", "已暂停"],
    blocked: ["Blocked", "受阻"],
    usage_limited: ["Usage limited", "用量受限"],
    budget_limited: ["Budget reached", "预算已用尽"],
    complete: ["Complete", "已完成"]
  };
  return t(...labels[status]);
}

/**
 * 将秒数格式化为紧凑时间。
 *
 * @param seconds 累计秒数
 * @returns 时间文本
 */
function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  return `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
}
