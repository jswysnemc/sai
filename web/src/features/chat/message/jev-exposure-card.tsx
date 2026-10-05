import { useState } from "react";
import { DiamondCheck } from "../../../shared/ui/icons";
import { useI18n } from "../../i18n/use-i18n";
import { ToolLayout } from "../tool-renderers/layout/tool-layout";
import type { JevCapabilityExposure } from "../tool-renderers/jev-capability-data";
import { isEmptyJevExposure, jevSelectionSummary } from "../tool-renderers/jev-capability-data";
import { JevResourceSections } from "../tool-renderers/jev-resource-sections";
import type { JevPreselectPhase } from "../run-event-reducer";
import "../tool-renderers/jev-capability-view.css";

type JevExposureCardProps = {
  phase: JevPreselectPhase;
  exposure: JevCapabilityExposure;
  detail?: string;
};

/**
 * 展示用户消息发往模型之前的自动 Jev 判断。
 *
 * 这张卡不属于模型后来的工具过程。折叠行用一句话说明 Jev 做了什么，
 * 展开后按工具、Skill、提示词片段、记忆分区列出，每条可再展开详情。
 *
 * @param props 判断阶段、暴露名单和失败摘要
 * @returns 发送前的 Jev 卡片
 */
export function JevExposureCard({ phase, exposure, detail = "" }: JevExposureCardProps) {
  const { locale, t } = useI18n();
  const [open, setOpen] = useState(false);
  const running = phase === "running";
  const failed = phase === "failed";
  const empty = phase === "empty" || (phase === "ready" && isEmptyJevExposure(exposure));
  const primary = running
    ? t("Picking tools and context for this message", "正在为本轮挑选工具与上下文")
    : failed
      ? t("Could not pick tools; using base tools only", "判断失败，本轮只用基础工具")
      : empty
        ? t("No extra tools or context needed", "本轮无需额外的工具或上下文")
        : jevSelectionSummary(exposure, locale);

  return (
    <ToolLayout
      icon={<DiamondCheck size={14} />}
      kindLabel="Jev"
      primaryText={primary}
      isRunning={running}
      canToggle={!running}
      expanded={open}
      onToggle={() => setOpen((value) => !value)}
      showFailureStatus={failed}
      title={failed && detail ? detail : undefined}
    >
      <div className="jev-capability">
        {failed && detail ? <p className="jev-capability-empty">{detail}</p> : null}
        {empty && !failed ? (
          <p className="jev-capability-empty">
            {t(
              "Jev selected no new tools, skills, prompt segments or memory for this message.",
              "Jev 没有为本轮新选中工具、Skill、提示词片段或记忆。"
            )}
          </p>
        ) : null}
        {!empty && !failed ? <JevResourceSections exposure={exposure} /> : null}
      </div>
    </ToolLayout>
  );
}
