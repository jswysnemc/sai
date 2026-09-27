import { useI18n } from "../../i18n/use-i18n";
import { ToolPanel } from "./layout/tool-panel";
import type { JevCapabilityExposure, JevExposedResource } from "./jev-capability-data";
import { parseJsonRecord, stringField } from "./tool-data";
import "./jev-capability-view.css";

type JevCapabilityViewProps = {
  argumentsText: string;
  exposure: JevCapabilityExposure;
};

/**
 * 渲染 Jev 能力申请展开后的暴露名单。
 *
 * 头部已经承担需求摘要和计数，这里只列出名称与一句说明，
 * 不重复工具 Schema 和 Skill 全文。
 *
 * @param props 申请参数与已解析的暴露结果
 * @returns 暴露名单
 */
export function JevCapabilityView({ argumentsText, exposure }: JevCapabilityViewProps) {
  const { t } = useI18n();
  const need = stringField(parseJsonRecord(argumentsText), "need");
  const empty = exposure.tools.length === 0 && exposure.skills.length === 0;
  return (
    <ToolPanel className="jev-capability-view">
      <div className="jev-capability">
        {need && <p className="jev-capability-need">{need}</p>}
        {empty ? (
          <p className="jev-capability-empty">
            {t("Jev did not expose another tool or skill.", "Jev 没有再暴露工具或 Skill。")}
          </p>
        ) : (
          <>
            <ResourceList
              title={t("Tools", "工具")}
              items={exposure.tools}
              detailLabel={(item) => item.detail}
            />
            <ResourceList
              title={t("Skills", "Skills")}
              items={exposure.skills}
              detailLabel={(item) => skillStatusLabel(item.detail, t)}
            />
          </>
        )}
      </div>
    </ToolPanel>
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
          const detail = detailLabel(item);
          return (
            <li key={`${item.kind}:${item.name}`}>
              <strong>{item.name}</strong>
              {detail ? <span>{detail}</span> : null}
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
