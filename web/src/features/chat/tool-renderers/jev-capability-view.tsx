import { useI18n } from "../../i18n/use-i18n";
import { ToolPanel } from "./layout/tool-panel";
import { isEmptyJevExposure, type JevCapabilityExposure } from "./jev-capability-data";
import { JevResourceSections } from "./jev-resource-sections";
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
  return (
    <ToolPanel className="jev-capability-view">
      <div className="jev-capability">
        {need && <p className="jev-capability-need">{need}</p>}
        {isEmptyJevExposure(exposure) ? (
          <p className="jev-capability-empty">
            {t("Jev did not expose another tool or skill.", "Jev 没有再暴露工具或 Skill。")}
          </p>
        ) : (
          <JevResourceSections exposure={exposure} />
        )}
      </div>
    </ToolPanel>
  );
}
