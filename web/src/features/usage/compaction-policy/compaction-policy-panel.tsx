import { useMutation } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "../../../api/client";
import { useI18n } from "../../i18n/use-i18n";
import { formatTokenCount } from "../token-format";
import {
  clampRatioPercent,
  clampReserveTokens,
  DEFAULT_RATIO_PERCENT,
  DEFAULT_RESERVE_TOKENS,
  MAX_RATIO_PERCENT,
  MIN_RATIO_PERCENT,
  parseReserveInput,
  RESERVE_PRESETS,
  resolveTriggerBreakdown
} from "./compaction-trigger";
import "./compaction-policy-panel.css";

type CompactionPolicyPanelProps = {
  sessionId: string;
  /** 当前生效的压缩比例，0~1 */
  ratio: number;
  /** 当前生效的预留 token */
  reserve: number;
  /** 上下文窗口总量 */
  windowTokens: number;
  /** 当前上下文占用 */
  usedTokens: number;
  /** 是否为本会话覆盖，false 表示沿用工作区默认 */
  overridden: boolean;
  onSaved: () => void;
};

/**
 * 会话级自动压缩策略面板。
 *
 * 比例与预留是并列条件、取更晚到达者，改一个未必影响结果。面板因此把
 * 两个条件各自的触发点摊开，标出当前实际生效的那个，避免用户调了半天
 * 没反应还找不到原因。
 *
 * @param props 当前策略、窗口占用与保存回调
 * @returns 压缩策略面板
 */
export function CompactionPolicyPanel({
  sessionId,
  ratio,
  reserve,
  windowTokens,
  usedTokens,
  overridden,
  onSaved
}: CompactionPolicyPanelProps) {
  const { t } = useI18n();
  const [ratioPercent, setRatioPercent] = useState(() => Math.round(ratio * 100));
  const [reserveText, setReserveText] = useState(() => String(reserve));

  const save = useMutation({
    mutationFn: (input: { compaction_ratio?: number; compaction_reserve_tokens?: number; reset?: boolean }) =>
      api.sessions.updateCompactionPolicy(sessionId, input),
    onSuccess: onSaved
  });

  // 后端返回新值后同步回本地，覆盖用户尚未提交的草稿
  useEffect(() => {
    setRatioPercent(Math.round(ratio * 100));
    setReserveText(String(reserve));
  }, [ratio, reserve]);

  // 用本地草稿实时预览，不必等保存往返
  const draftReserve = parseReserveInput(reserveText);
  const previewReserve = draftReserve == null ? reserve : clampReserveTokens(draftReserve, windowTokens);
  const breakdown = resolveTriggerBreakdown(windowTokens, clampRatioPercent(ratioPercent) / 100, previewReserve);
  const triggerPercent = windowTokens > 0 ? (breakdown.trigger / windowTokens) * 100 : 0;
  const usedPercent = windowTokens > 0 ? Math.min(100, (usedTokens / windowTokens) * 100) : 0;
  const headroom = Math.max(0, breakdown.trigger - usedTokens);

  /** 提交比例，与当前生效值相同则跳过请求 */
  const commitRatio = (percent: number) => {
    const next = clampRatioPercent(percent);
    setRatioPercent(next);
    if (next !== Math.round(ratio * 100)) save.mutate({ compaction_ratio: next / 100 });
  };

  /** 提交预留，解析失败则回滚到当前生效值 */
  const commitReserve = (raw: string) => {
    const parsed = parseReserveInput(raw);
    if (parsed == null) {
      setReserveText(String(reserve));
      return;
    }
    const next = clampReserveTokens(parsed, windowTokens);
    setReserveText(String(next));
    if (next !== reserve) save.mutate({ compaction_reserve_tokens: next });
  };

  return (
    <section className="compaction-policy">
      <header className="compaction-policy-head">
        <div>
          <span>{t("Session auto-compact", "本会话自动压缩")}</span>
          <small>
            {overridden
              ? t("Overrides the workspace default", "覆盖工作区默认")
              : t("Using workspace default", "使用工作区默认")}
          </small>
        </div>
        {overridden ? (
          <button
            type="button"
            className="compaction-policy-reset"
            onClick={() => save.mutate({ reset: true })}
            disabled={save.isPending}
          >
            {t("Reset", "恢复默认")}
          </button>
        ) : null}
      </header>

      <div className="compaction-policy-preview">
        <p>
          {t("Compacts at", "将在")}
          <strong>{formatTokenCount(breakdown.trigger)}</strong>
          {t(`· ${triggerPercent.toFixed(1)}% of window`, `触发 · 占窗口 ${triggerPercent.toFixed(1)}%`)}
        </p>
        <div
          className="compaction-policy-track"
          role="img"
          aria-label={t(
            `Currently ${formatTokenCount(usedTokens)} of ${formatTokenCount(windowTokens)}, compacts at ${formatTokenCount(breakdown.trigger)}`,
            `当前 ${formatTokenCount(usedTokens)}，窗口 ${formatTokenCount(windowTokens)}，触发于 ${formatTokenCount(breakdown.trigger)}`
          )}
        >
          <span className="compaction-policy-track-used" style={{ width: `${usedPercent}%` }} />
          <span className="compaction-policy-track-mark" style={{ left: `${Math.min(100, triggerPercent)}%` }} />
        </div>
        <div className="compaction-policy-scale">
          <small>{t("Now", "现在")} {formatTokenCount(usedTokens)}</small>
          <small>
            {headroom > 0
              ? t(`${formatTokenCount(headroom)} to go`, `距触发还有 ${formatTokenCount(headroom)}`)
              : t("Threshold reached", "已达阈值")}
          </small>
          <small>{t("Window", "窗口")} {formatTokenCount(windowTokens)}</small>
        </div>
      </div>

      <div className="compaction-policy-conditions">
        <div className={`compaction-policy-condition${breakdown.active === "ratio" ? " is-active" : ""}`}>
          <div className="compaction-policy-condition-head">
            <span>{t("Usage reaches", "占用达到窗口的")}</span>
            <output>
              {formatTokenCount(breakdown.ratioTrigger)}
              {breakdown.active === "ratio" ? <em>{t("in effect", "生效")}</em> : null}
            </output>
          </div>
          <div className="compaction-policy-condition-body">
            <input
              type="range"
              min={MIN_RATIO_PERCENT}
              max={MAX_RATIO_PERCENT}
              step={1}
              value={ratioPercent}
              aria-label={t("Compaction ratio percent", "压缩比例百分比")}
              onChange={(event) => setRatioPercent(Number(event.target.value))}
              onPointerUp={() => commitRatio(ratioPercent)}
              onKeyUp={() => commitRatio(ratioPercent)}
              disabled={save.isPending}
            />
            <label className="compaction-policy-number">
              <input
                type="number"
                min={MIN_RATIO_PERCENT}
                max={MAX_RATIO_PERCENT}
                step={1}
                value={ratioPercent}
                aria-label={t("Compaction ratio percent", "压缩比例百分比")}
                onChange={(event) => setRatioPercent(Number(event.target.value))}
                onBlur={() => commitRatio(ratioPercent)}
                disabled={save.isPending}
              />
              <span>%</span>
            </label>
          </div>
        </div>

        <div className={`compaction-policy-condition${breakdown.active === "reserve" ? " is-active" : ""}`}>
          <div className="compaction-policy-condition-head">
            <span>{t("Or headroom drops below", "或剩余空间不足")}</span>
            <output>
              {breakdown.reserveTrigger == null
                ? t("not applied", "不参与")
                : formatTokenCount(breakdown.reserveTrigger)}
              {breakdown.active === "reserve" ? <em>{t("in effect", "生效")}</em> : null}
            </output>
          </div>
          <div className="compaction-policy-condition-body">
            <label className="compaction-policy-number wide">
              <input
                type="text"
                inputMode="numeric"
                value={reserveText}
                aria-label={t("Reserved tokens", "预留 token")}
                onChange={(event) => setReserveText(event.target.value)}
                onBlur={(event) => commitReserve(event.target.value)}
                disabled={save.isPending}
              />
              <span>tokens</span>
            </label>
            <div className="compaction-policy-presets">
              {RESERVE_PRESETS.map((preset) => (
                <button
                  type="button"
                  key={preset}
                  className={preset === previewReserve ? "is-selected" : ""}
                  onClick={() => commitReserve(String(preset))}
                  disabled={save.isPending}
                >
                  {preset === 0 ? t("Off", "关闭") : formatTokenCount(preset)}
                </button>
              ))}
            </div>
          </div>
        </div>

        <p className="compaction-policy-note">
          {breakdown.reserveTrigger == null && previewReserve > 0
            ? t(
                "Reserve exceeds the window, so only the ratio applies.",
                "预留超过窗口，本次只按比例触发。"
              )
            : t(
                "Whichever condition is reached later wins.",
                "两个条件取更晚到达的那个。"
              )}
        </p>
      </div>

      {save.error ? <p className="compaction-policy-error">{save.error.message}</p> : null}
    </section>
  );
}

/** 默认策略，供调用方在后端字段缺失时兜底 */
export const COMPACTION_POLICY_FALLBACK = {
  ratio: DEFAULT_RATIO_PERCENT / 100,
  reserve: DEFAULT_RESERVE_TOKENS
};
