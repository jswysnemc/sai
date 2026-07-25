import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronDown, Pencil, Play, Pause, Target, Trash2, X } from "lucide-react";
import { useMemo, useState } from "react";
import { api } from "../../api/client";
import type { Goal, GoalStatus, GoalUpdateEntry } from "../../api/goal-contracts";
import { toDisplayError } from "../../api/api-error";
import { Button } from "../../shared/ui/button/button";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { parseComposerAtoms } from "../chat/composer/composer-atom-token";
import { LightboxImage } from "../../shared/ui/image-lightbox";
import { useI18n } from "../i18n/use-i18n";
import "./goal-control.css";

type GoalControlProps = {
  sessionId?: string;
  running: boolean;
  /** 底部输入框当前草稿 */
  draftValue: string;
  /** 更新底部输入框草稿 */
  onDraftChange: (value: string) => void;
  onContinue: () => Promise<void>;
};

/**
 * 目标入口：
 * 1. 靶心按钮只切换底部输入的 `/goal` 前缀，不再打开编辑弹层
 * 2. 已有目标时展示可展开标记，支持查看正文、取消、编辑落回输入框
 *
 * @param props 会话、运行状态、草稿与续轮回调
 * @returns Goal 控件
 */
export function GoalControl({ sessionId, running, draftValue, onDraftChange, onContinue }: GoalControlProps) {
  const { t, locale } = useI18n();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const [expanded, setExpanded] = useState(false);
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
  const goalModeActive = useMemo(() => hasGoalPrefix(draftValue), [draftValue]);

  /**
   * 写入 Goal 查询缓存。
   *
   * @param next 新目标
   */
  const cacheGoal = (next: Goal | null) => {
    queryClient.setQueryData(queryKey, { goal: next });
  };

  /**
   * 切换输入框开头的 goal 模式前缀。
   */
  const toggleGoalPrefix = () => {
    if (!sessionId) return;
    onDraftChange(toggleGoalMode(draftValue));
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
      setExpanded(false);
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to clear goal", "清除目标失败"));
    } finally {
      setBusy(false);
    }
  };

  /**
   * 将当前目标正文落回底部输入框，并加上 `/goal` 前缀以便再次发送。
   */
  const editGoal = async () => {
    if (!goal) return;
    const accepted = await confirm({
      title: t("Edit goal in composer?", "在输入框中编辑目标？"),
      description: t(
        "The goal text will replace the current composer draft. Attachments and skills already in the objective stay as text.",
        "目标正文会替换当前输入草稿；目标中的附件与技能会以文本形式保留。"
      ),
      confirmLabel: t("Edit", "编辑")
    });
    if (!accepted) return;
    // 编辑时去掉超长 data URL 图片，保留文本/技能/文件原子，避免撑爆输入框
    const editable = goal.objective.replace(/!\[[^\]]*\]\(data:[^)]+\)/gu, "").trim();
    const next = ensureGoalPrefix(editable || goal.objective.split("\n").find((line) => !line.includes("data:")) || "");
    onDraftChange(next);
    setExpanded(false);
  };

  const statusLabel = goal ? goalStatusLabel(goal.status, t) : t("No active goal", "暂无目标");
  const updates = [...(goal?.updates ?? [])].reverse();
  const preview = goal ? parseGoalPreview(goal.objective) : null;

  return (
    <div className={`goal-control${goal ? ` status-${goal.status}` : ""}${goalModeActive ? " goal-mode-on" : ""}`}>
      <div className={`goal-marker${expanded ? " is-expanded" : ""}${goalModeActive ? " is-mode-on" : ""}`}>
        {/* 与模型选择/YOLO 同为无边框文本按钮；有目标时只显示状态文字 */}
        <button
          type="button"
          className={`composer-rail-button goal-control-trigger${goal ? " has-goal" : ""}${goalModeActive ? " is-active" : ""}`}
          onClick={() => {
            if (goal) {
              setExpanded((value) => !value);
              return;
            }
            toggleGoalPrefix();
          }}
          disabled={!sessionId}
          title={
            goal
              ? `${statusLabel}: ${preview?.text || goal.objective}`
              : goalModeActive
                ? t("Remove goal mode from input", "从输入中去掉 goal 模式")
                : t("Add goal mode to input", "在输入前加上 goal 模式")
          }
          aria-label={
            goal
              ? t("Toggle goal details", "展开或收起目标详情")
              : goalModeActive
                ? t("Remove goal mode from input", "从输入中去掉 goal 模式")
                : t("Add goal mode to input", "在输入前加上 goal 模式")
          }
          aria-expanded={goal ? expanded : undefined}
          aria-pressed={!goal ? goalModeActive : undefined}
        >
          <Target size={14} aria-hidden />
          {goal && (
            <>
              <span className="goal-marker-label">{statusLabel}</span>
              <ChevronDown size={12} className={`goal-marker-chevron${expanded ? " rotate" : ""}`} aria-hidden />
            </>
          )}
        </button>

        {goal && goal.status === "active" && !running && (
          <Button className="composer-rail-button goal-control-action" onClick={() => void continueGoal()} disabled={busy} title={t("Continue goal", "继续目标")} aria-label={t("Continue goal", "继续目标")}>
            <Play size={13} />
          </Button>
        )}
        {goal && goal.status === "active" && (
          <Button className="composer-rail-button goal-control-action" onClick={() => void updateStatus("paused")} disabled={busy} title={t("Pause goal", "暂停目标")} aria-label={t("Pause goal", "暂停目标")}>
            <Pause size={13} />
          </Button>
        )}
        {goal && ["paused", "blocked", "usage_limited"].includes(goal.status) && (
          <Button className="composer-rail-button goal-control-action" onClick={() => void updateStatus("active")} disabled={busy || running} title={t("Resume goal", "恢复目标")} aria-label={t("Resume goal", "恢复目标")}>
            <Play size={13} />
          </Button>
        )}

        {goal && expanded && (
          <div className="goal-marker-panel" role="region" aria-label={t("Goal details", "目标详情")}>
            <header className="goal-marker-panel-head">
              <strong>{t("Session goal", "会话目标")}</strong>
              <span className={`goal-status-badge status-${goal.status}`}>{statusLabel}</span>
              <Button className="goal-marker-close composer-rail-button" onClick={() => setExpanded(false)} aria-label={t("Close", "关闭")} title={t("Close", "关闭")}>
                <X size={12} />
              </Button>
            </header>

            <div className="goal-marker-objective">
              {preview?.skills.map((name) => (
                <span className="goal-marker-chip skill" key={`skill-${name}`}>/{name}</span>
              ))}
              {preview?.files.map((file) => (
                <span className="goal-marker-chip file" key={`file-${file}`}>{file}</span>
              ))}
              {preview?.terminals.map((item, index) => (
                <span className="goal-marker-chip terminal" key={`term-${index}`}>{item}</span>
              ))}
              {preview?.images && preview.images.length > 0 && (
                <div className="goal-marker-images">
                  {preview.images.map((src, index) => (
                    <LightboxImage key={`${src.slice(0, 24)}-${index}`} src={src} alt={t(`Goal image ${index + 1}`, `目标图片 ${index + 1}`)} />
                  ))}
                </div>
              )}
              <p>{preview?.text || goal.objective}</p>
            </div>

            <GoalUsage goal={goal} />

            {updates.length > 0 && (
              <section className="goal-updates compact">
                <header>
                  <strong>{t("Progress log", "执行更新")}</strong>
                  <small>{t(`${updates.length} entries`, `${updates.length} 条`)}</small>
                </header>
                <ol className="goal-updates-list">
                  {updates
                    .map((entry) => sanitizeGoalUpdateEntry(entry, t))
                    .filter((entry) => entry.message.trim().length > 0)
                    .slice(0, 8)
                    .map((entry, index) => (
                      <GoalUpdateItem key={`${entry.at}-${index}`} entry={entry} locale={locale} t={t} />
                    ))}
                </ol>
              </section>
            )}

            <div className="goal-marker-actions">
              <Button variant="danger" onClick={() => void clear()} disabled={busy} title={t("Cancel goal", "取消目标")}>
                <Trash2 size={13} />
                <span>{t("Cancel", "取消目标")}</span>
              </Button>
              <Button onClick={() => void editGoal()} disabled={busy} title={t("Edit goal", "编辑目标")}>
                <Pencil size={13} />
                <span>{t("Edit", "编辑")}</span>
              </Button>
            </div>

            {(error || goalQuery.error) && (
              <div className="goal-control-error">{error?.message ?? goalQuery.error?.message}</div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

/**
 * 判断草稿是否处于 goal 模式。
 *
 * @param value 输入草稿
 * @returns 是否以 /goal 前缀开始
 */
export function hasGoalPrefix(value: string): boolean {
  return /^\/goal(?:\s|$)/iu.test(value.trimStart());
}

/**
 * 确保草稿以 `/goal` 前缀开始。
 *
 * @param value 目标正文或草稿
 * @returns 带前缀的草稿
 */
export function ensureGoalPrefix(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) return "/goal ";
  if (hasGoalPrefix(trimmed)) return trimmed.endsWith("\n") || trimmed.endsWith(" ") ? trimmed : `${trimmed} `;
  return `/goal ${trimmed}`;
}

/**
 * 切换输入草稿的 goal 模式前缀。
 *
 * @param value 当前草稿
 * @returns 切换后的草稿
 */
export function toggleGoalMode(value: string): string {
  const leading = value.match(/^\s*/u)?.[0] ?? "";
  const body = value.slice(leading.length);
  if (/^\/goal(?:\s|$)/iu.test(body)) {
    return leading + body.replace(/^\/goal(?:[\s\u00a0\u3000]+)?/iu, "");
  }
  if (!body) return `${leading}/goal `;
  return `${leading}/goal ${body}`;
}

/**
 * 解析目标正文中的技能、文件、终端选区和纯文本。
 *
 * @param objective 目标原文
 * @returns 预览结构
 */
function parseGoalPreview(objective: string) {
  const segments = parseComposerAtoms(objective);
  const skills: string[] = [];
  const files: string[] = [];
  const terminals: string[] = [];
  const images: string[] = [];
  let text = "";
  for (const segment of segments) {
    if (segment.type === "text") {
      text += segment.value;
      continue;
    }
    if (segment.type === "goal") continue;
    if (segment.type === "skill") {
      skills.push(segment.name);
      continue;
    }
    if (segment.type === "file") {
      files.push(segment.path);
      continue;
    }
    if (segment.type === "terminal") {
      terminals.push(segment.source || "Terminal");
    }
  }
  // 1. 抽出 markdown 图片（含 data URL），正文中移除对应标记
  const imagePattern = /!\[[^\]]*\]\(([^)]+)\)/gu;
  for (const match of text.matchAll(imagePattern)) {
    const src = (match[1] ?? "").trim();
    if (src) images.push(src);
  }
  text = text.replace(imagePattern, "").replace(/^\/goal\s*/iu, "").trim();
  return {
    text,
    skills,
    files,
    terminals,
    images
  };
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
      <p title={entry.message}>{entry.message}</p>
    </li>
  );
}

/**
 * 清洗历史 updates，去掉附件 data URL 与超长 base64 残骸。
 *
 * @param entry 原始更新
 * @param t 本地化函数
 * @returns 可展示的更新
 */
function sanitizeGoalUpdateEntry(
  entry: GoalUpdateEntry,
  t: (en: string, zh: string) => string
): GoalUpdateEntry {
  // 1. 去掉 markdown 图片 data URL 与裸 data URL
  // 2. 压缩空白并截断，避免执行更新区被 base64 撑开
  let message = stripDataUrls(entry.message).replace(/\s+/gu, " ").trim();
  if (!message) {
    message = entry.kind === "status"
      ? t("Status updated", "状态已更新")
      : entry.kind === "account"
        ? t("Usage updated", "用量已更新")
        : t("Progress updated", "进度已更新");
  }
  if (message.length > 220) {
    message = `${message.slice(0, 220)}…`;
  }
  return { ...entry, message };
}

/**
 * 从文本中移除 data URL 附件内容。
 *
 * @param value 原始文本
 * @returns 清洗后的文本
 */
function stripDataUrls(value: string): string {
  let text = value;
  // 1. 删除 markdown 图片 data URL：`![alt](data:...)`
  let cursor = 0;
  let rebuilt = "";
  while (cursor < text.length) {
    const start = text.indexOf("![", cursor);
    if (start < 0) {
      rebuilt += text.slice(cursor);
      break;
    }
    rebuilt += text.slice(cursor, start);
    const altEnd = text.indexOf("](", start);
    if (altEnd < 0) {
      rebuilt += text.slice(start);
      break;
    }
    const urlStart = altEnd + 2;
    if (!text.startsWith("data:", urlStart)) {
      rebuilt += text.slice(start, urlStart);
      cursor = urlStart;
      continue;
    }
    const urlEnd = text.indexOf(")", urlStart);
    if (urlEnd < 0) {
      rebuilt += text.slice(start);
      break;
    }
    cursor = urlEnd + 1;
  }
  text = rebuilt;

  // 2. 删除裸 data: URL
  rebuilt = "";
  cursor = 0;
  while (cursor < text.length) {
    const start = text.indexOf("data:", cursor);
    if (start < 0) {
      rebuilt += text.slice(cursor);
      break;
    }
    rebuilt += text.slice(cursor, start);
    let end = start + "data:".length;
    while (end < text.length) {
      const ch = text[end] ?? "";
      if (/\s/u.test(ch) || ch === ")" || ch === "]" || ch === "<" || ch === ">" || ch === "\"" || ch === "'") break;
      end += 1;
    }
    cursor = end;
  }
  return rebuilt;
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
