import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Pause, Play, Target, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../api/client";
import type { Goal, GoalStatus, GoalUpdateEntry } from "../../api/goal-contracts";
import { toDisplayError } from "../../api/api-error";
import { Button } from "../../shared/ui/button/button";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { Modal } from "../../shared/ui/dialog/modal";
import { ComposerTextarea } from "../chat/composer/composer-textarea";
import { useI18n } from "../i18n/use-i18n";
import "../chat/chat-composer.css";
import "./goal-control.css";

type GoalControlProps = {
  sessionId?: string;
  running: boolean;
  onContinue: () => Promise<void>;
};

/**
 * 目标入口：查看详情、清除、暂停/继续，以及切换新目标。
 * 编辑区复用底部 ComposerTextarea（@ 文件、/ 命令、原子、历史、粘贴），底部按钮改为目标操作。
 *
 * @param props 会话标识、运行状态和续轮回调
 * @returns Goal 控件
 */
export function GoalControl({ sessionId, running, onContinue }: GoalControlProps) {
  const { t, locale } = useI18n();
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
    refetchInterval: (query) => (query.state.data?.goal?.status === "active" ? 2_000 : false)
  });
  const goal = goalQuery.data?.goal ?? null;

  useEffect(() => {
    if (!open) return;
    setObjective(goal?.objective ?? "");
    setTokenBudget(goal?.token_budget ? String(goal.token_budget) : "");
    setError(null);
  }, [goal?.id, goal?.objective, goal?.token_budget, open]);

  /** 打开目标详情弹层。 */
  const openGoal = () => {
    setError(null);
    setOpen(true);
  };

  /**
   * 写入 Goal 查询缓存。
   *
   * @param next 新目标
   */
  const cacheGoal = (next: Goal | null) => {
    queryClient.setQueryData(queryKey, { goal: next });
  };

  /**
   * 从编辑器内容提取目标正文。
   * 允许用户写 `/goal 内容` 或直接写内容。
   *
   * @param value 编辑器纯文本
   * @returns 目标正文
   */
  const resolveObjective = (value: string): string => {
    const trimmed = value.trim();
    const match = trimmed.match(/^\/goal(?:\s+([\s\S]*))?$/u);
    if (match) return (match[1] ?? "").trim();
    return trimmed;
  };

  /**
   * 创建或切换目标并启动续轮。
   */
  const save = async () => {
    if (!sessionId) return;
    const nextObjective = resolveObjective(objective);
    if (!nextObjective) {
      setError(new Error(t("Enter a goal objective", "请输入目标内容")));
      return;
    }
    const parsedBudget = tokenBudget.trim() ? Number(tokenBudget) : undefined;
    if (parsedBudget !== undefined && (!Number.isSafeInteger(parsedBudget) || parsedBudget <= 0)) {
      setError(new Error(t("Token budget must be a positive integer", "Token 预算必须为正整数")));
      return;
    }
    if (goal && nextObjective === goal.objective && (parsedBudget ?? null) === (goal.token_budget ?? null)) {
      await continueGoal();
      setOpen(false);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      // 1. Web 显式切换允许替换未结束目标
      const response = await api.goals.set(sessionId, nextObjective, parsedBudget);
      cacheGoal(response.goal ?? null);
      // 2. 空闲时立即续轮
      if (!running) await onContinue();
      setOpen(false);
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to save goal", "保存目标失败"));
    } finally {
      setBusy(false);
    }
  };

  /**
   * 更新目标状态。
   *
   * @param status 新状态
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

  /** 启动空闲目标续轮。 */
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

  /** 确认并清除当前目标。 */
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
      setObjective("");
      setTokenBudget("");
      setOpen(false);
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to clear goal", "清除目标失败"));
    } finally {
      setBusy(false);
    }
  };

  /**
   * 目标编辑区忽略图片粘贴：目标正文仅支持文本与输入原子。
   */
  const ignorePasteImages = async () => undefined;

  const statusLabel = goal ? goalStatusLabel(goal.status, t) : t("No active goal", "暂无目标");
  const updates = [...(goal?.updates ?? [])].reverse();

  return (
    <>
      <div className={`goal-control${goal ? ` status-${goal.status}` : ""}`}>
        <Button
          className="composer-rail-button goal-control-trigger"
          onClick={openGoal}
          disabled={!sessionId}
          title={goal?.objective ?? t("Goal details", "目标详情")}
          aria-label={goal ? t("Open goal details", "打开目标详情") : t("Open goal panel", "打开目标面板")}
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
        title={t("Session goal", "会话目标")}
        description={statusLabel}
        size="medium"
        onClose={() => setOpen(false)}
      >
        <div className="goal-control-form">
          <div className="goal-composer-shell">
            <div className="composer goal-composer" data-goal-editor="true">
              <ComposerTextarea
                value={objective}
                historyEntries={[]}
                disabled={busy}
                placeholder={t("Describe the objective. @ files, / commands and Enter to save.", "描述目标。支持 @ 文件、/ 命令，Enter 保存。")}
                onChange={setObjective}
                onPasteImages={ignorePasteImages}
                onSubmit={() => {
                  void save();
                }}
              />
              <div className="composer-footer goal-composer-footer">
                <div className="composer-toolrail">
                  <label className="goal-budget-field">
                    <span>{t("Token budget", "Token 预算")}</span>
                    <input
                      type="number"
                      min={1}
                      step={1}
                      value={tokenBudget}
                      onChange={(event) => setTokenBudget(event.target.value)}
                      placeholder={t("Unlimited", "不限")}
                      disabled={busy}
                    />
                  </label>
                </div>
                <div className="composer-actions goal-composer-actions">
                  {goal && (
                    <Button variant="danger" onClick={() => void clear()} disabled={busy} title={t("Clear", "清除")}>
                      <Trash2 size={14} />
                    </Button>
                  )}
                  {goal?.status === "active" && (
                    <Button onClick={() => void updateStatus("paused")} disabled={busy} title={t("Pause", "暂停")}>
                      <Pause size={14} />
                    </Button>
                  )}
                  {goal && ["paused", "blocked", "usage_limited"].includes(goal.status) && (
                    <Button onClick={() => void updateStatus("active")} disabled={busy || running} title={t("Resume", "恢复")}>
                      <Play size={14} />
                    </Button>
                  )}
                  <Button variant="primary" onClick={() => void save()} disabled={busy || !resolveObjective(objective)}>
                    {goal ? t("Switch / continue", "切换 / 继续") : t("Start goal", "开始目标")}
                  </Button>
                </div>
              </div>
            </div>
            <p className="goal-composer-hint">
              {t(
                "Same editor as chat: @ files, /goal and skills, history keys. Images are ignored for goals.",
                "与聊天输入相同：@ 文件、/goal 与 Skills、方向键历史。目标不接收图片。"
              )}
            </p>
          </div>

          {goal && <GoalUsage goal={goal} />}

          {goal && (
            <section className="goal-updates">
              <header>
                <strong>{t("Progress log", "执行更新")}</strong>
                <small>{t(`${updates.length} entries`, `${updates.length} 条`)}</small>
              </header>
              {updates.length === 0 ? (
                <div className="goal-updates-empty">{t("No turn updates yet", "尚无轮次更新")}</div>
              ) : (
                <ol className="goal-updates-list">
                  {updates.map((entry, index) => (
                    <GoalUpdateItem key={`${entry.at}-${index}`} entry={entry} locale={locale} t={t} />
                  ))}
                </ol>
              )}
            </section>
          )}

          {(error || goalQuery.error) && (
            <div className="goal-control-error">{error?.message ?? goalQuery.error?.message}</div>
          )}
        </div>
      </Modal>
    </>
  );
}

/**
 * 渲染单条目标更新。
 */
function GoalUpdateItem({
  entry,
  locale,
  t
}: {
  entry: GoalUpdateEntry;
  locale: string;
  t: (en: string, zh: string) => string;
}) {
  return (
    <li className={`goal-update kind-${entry.kind}`}>
      <div className="goal-update-head">
        <span className="goal-update-kind">{kindLabel(entry.kind, t)}</span>
        <time dateTime={entry.at}>{formatTime(entry.at, locale)}</time>
      </div>
      <p>{entry.message}</p>
    </li>
  );
}

/**
 * 渲染目标用量摘要。
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

function kindLabel(kind: string, t: (en: string, zh: string) => string): string {
  if (kind === "progress") return t("Progress", "进度");
  if (kind === "status") return t("Status", "状态");
  if (kind === "account") return t("Usage", "用量");
  return kind;
}

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

function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  return `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
}

function formatTime(value: string, locale: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(locale.startsWith("zh") ? "zh-CN" : "en-US", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit"
  });
}
