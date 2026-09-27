import { useState } from "react";
import { DiamondCheck } from "../../../shared/ui/icons";
import { useI18n } from "../../i18n/use-i18n";
import { ToolLayout } from "../tool-renderers/layout/tool-layout";
import type { JevCapabilityExposure, JevExposedResource } from "../tool-renderers/jev-capability-data";
import { jevCapabilityStatusLabel } from "../tool-renderers/jev-capability-data";
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
 * 这张卡不属于模型后来的工具过程。折叠行给出名称或结论，
 * 展开后只列说明首句和 Skill 状态。
 *
 * @param props 判断阶段、暴露名单和失败摘要
 * @returns 发送前的 Jev 卡片
 */
export function JevExposureCard({ phase, exposure, detail = "" }: JevExposureCardProps) {
  const { locale, t } = useI18n();
  const [open, setOpen] = useState(false);
  const names = [
    ...exposure.tools.map((item) => item.name),
    ...exposure.skills.map((item) => `skill:${item.name}`)
  ];
  const running = phase === "running";
  const failed = phase === "failed";
  const empty = phase === "empty" || (phase === "ready" && names.length === 0);
  const primary = running
    ? t("Choosing tools for this message", "正在判断本轮要暴露的工具")
    : failed
      ? t("Judgment failed; only base tools stay available", "判断失败，本轮只用基础工具")
      : empty
        ? t("No new tools or skills", "没有新的工具或 Skill")
        : names.join(", ");

  return (
    <ToolLayout
      icon={<DiamondCheck size={14} />}
      kindLabel="Jev"
      kindDetail={t("before send", "发送前")}
      primaryText={primary}
      statusLabel={phase === "ready" && !empty ? jevCapabilityStatusLabel(exposure, locale) : undefined}
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
            {t("Jev exposed nothing new for this message.", "Jev 没有为本轮新暴露工具或 Skill。")}
          </p>
        ) : null}
        {!empty && !failed ? (
          <>
            <ResourceList title={t("Tools", "工具")} items={exposure.tools} detailLabel={(item) => item.detail} />
            <ResourceList
              title="Skills"
              items={exposure.skills}
              detailLabel={(item) => skillStatusLabel(item.detail, t)}
            />
          </>
        ) : null}
      </div>
    </ToolLayout>
  );
}

/**
 * 渲染一类资源的名称列表。
 *
 * @param props 标题、条目，以及把 detail 转成界面文字的方法
 * @returns 列表；没有条目时为空
 */
function ResourceList({
  title,
  items,
  detailLabel
}: {
  title: string;
  items: JevExposedResource[];
  detailLabel: (item: JevExposedResource) => string;
}) {
  if (items.length === 0) return null;
  return (
    <section>
      <h3>{title}</h3>
      <ul>
        {items.map((item) => {
          const detailText = detailLabel(item);
          return (
            <li key={`${item.kind}:${item.name}`}>
              <strong>{item.name}</strong>
              {detailText ? <span>{detailText}</span> : null}
            </li>
          );
        })}
      </ul>
    </section>
  );
}

/**
 * 把 skill 状态码转成界面文字。
 *
 * @param status 后端状态
 * @param t 双语文本选择方法
 * @returns 状态说明；未知状态原样返回
 */
function skillStatusLabel(status: string, t: (en: string, zh: string) => string): string {
  if (status === "loaded") return t("newly exposed", "新暴露");
  if (status === "already_loaded") return t("already exposed", "此前已暴露");
  return status;
}
