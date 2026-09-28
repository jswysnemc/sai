import { useI18n } from "../../i18n/use-i18n";
import { ChoicePills, FieldGrid, SettingsField, SettingsPanel, SwitchField } from "../kit";
import type { MarkdownStyleSettingsProps } from "./markdown-settings-types";

/**
 * 【Web 设置】【Markdown 外观】渲染领域独立的外观字段。
 * @param props 当前偏好与对应领域更新回调
 * @returns 外观设置面板
 */
export function MarkdownTableSettings({ preferences, onTableChange }: Pick<MarkdownStyleSettingsProps, "preferences" | "onTableChange">) {
  const { t } = useI18n();
  return (
    <SettingsPanel title={t("Tables", "表格")}>
      <FieldGrid>
        <SettingsField label={t("Table borders", "表格边框")} anchor="appearance.markdown.table.borderStyle">
          <ChoicePills
            value={preferences.table.borderStyle}
            options={[
              { value: "horizontal", label: t("Horizontal lines", "仅横向分隔线") },
              { value: "grid", label: t("Full grid", "完整网格") },
              { value: "none", label: t("No borders", "无边框") }
            ]}
            ariaLabel={t("Table borders", "表格边框")}
            onChange={(borderStyle) => onTableChange({ borderStyle })}
          />
        </SettingsField>
        <SettingsField label={t("Cell density", "单元格密度")} anchor="appearance.markdown.table.density">
          <ChoicePills
            value={preferences.table.density}
            options={[
              { value: "compact", label: t("Compact", "紧凑") },
              { value: "comfortable", label: t("Comfortable", "标准") },
              { value: "spacious", label: t("Spacious", "宽松") }
            ]}
            ariaLabel={t("Cell density", "单元格密度")}
            onChange={(density) => onTableChange({ density })}
          />
        </SettingsField>
        <SwitchField
          label={t("Full width", "占满内容宽度")}
          hint={t("Stretch short tables to the message width.", "短表格也扩展到消息内容宽度。")}
          checked={preferences.table.fullWidth}
          anchor="appearance.markdown.table.fullWidth"
          onChange={(fullWidth) => onTableChange({ fullWidth })}
        />
        <SwitchField
          label={t("Striped rows", "斑马纹")}
          hint={t("Add a subtle surface to alternating rows.", "为交替数据行增加轻微底色。")}
          checked={preferences.table.stripedRows}
          anchor="appearance.markdown.table.stripedRows"
          onChange={(stripedRows) => onTableChange({ stripedRows })}
        />
        <SwitchField
          label={t("Header background", "表头底色")}
          hint={t("Separate the header with a muted surface.", "使用克制底色区分表头。")}
          checked={preferences.table.headerBackground}
          anchor="appearance.markdown.table.headerBackground"
          onChange={(headerBackground) => onTableChange({ headerBackground })}
        />
        <SwitchField
          label={t("Wrap cell content", "单元格内容换行")}
          hint={t("Wrap long text instead of keeping every cell on one line.", "长文本可以换行，不强制每个单元格保持单行。")}
          checked={preferences.table.wrapCells}
          anchor="appearance.markdown.table.wrapCells"
          onChange={(wrapCells) => onTableChange({ wrapCells })}
        />
      </FieldGrid>
    </SettingsPanel>
  );
}
