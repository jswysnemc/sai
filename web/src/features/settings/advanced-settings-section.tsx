import { useEffect, useState } from "react";
import type { AppConfig } from "../../api/contracts";
import { EditorHeader } from "./editor-layout";
import { JsonCodeEditor } from "../../shared/ui/code-editor/json-code-editor";
import { useI18n } from "../i18n/use-i18n";
import "./advanced-settings-section.css";

type AdvancedSettingsSectionProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染完整 AppConfig JSON 编辑器。
 *
 * JSON 文本为本分区私有状态：合法输入即时解析并写入全局草稿走
 * 顶栏保存；非法输入只在本地提示，不污染草稿。其他分区修改配置后
 * 切回本页时按最新草稿重建文本。
 *
 * @param props 当前配置草稿与更新回调
 * @returns 高级设置区域
 */
export function AdvancedSettingsSection({ config, onConfigChange }: AdvancedSettingsSectionProps) {
  const { t } = useI18n();
  const [raw, setRaw] = useState(() => JSON.stringify(config, null, 2));
  const [parseError, setParseError] = useState<string | null>(null);

  useEffect(() => {
    // 1. 本地编辑回环（文本与草稿等价）不重排用户排版；
    //    外部草稿变化（其他分区编辑、保存回填）时重建文本
    try {
      if (JSON.stringify(JSON.parse(raw)) === JSON.stringify(config)) return;
    } catch {
      // 文本当前非法：草稿变了也重建，非法片段本就无法保存
    }
    setRaw(JSON.stringify(config, null, 2));
    setParseError(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [config]);

  /**
   * 更新 JSON 文本；合法时同步到全局草稿。
   *
   * @param value 编辑器文本
   * @returns 无返回值
   */
  const handleChange = (value: string) => {
    setRaw(value);
    try {
      const parsed = JSON.parse(value) as AppConfig;
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
        throw new Error("AppConfig must be a JSON object");
      }
      setParseError(null);
      onConfigChange(parsed);
    } catch (error) {
      // 2. 输入中途不合法：保留文本与上一份草稿，仅提示
      setParseError(error instanceof Error ? error.message : String(error));
    }
  };

  return (
    <section className="settings-editor advanced-settings">
      <EditorHeader kicker={t("Complete configuration", "完整配置")} title={t("Advanced JSON", "高级 JSON")} description={t("When saving, the server deserializes the configuration again, merges sensitive fields, and performs full validation.", "保存时服务端会重新反序列化、合并敏感字段并执行完整校验。")} />
      <div className="advanced-settings-note">{t("Edit tool, display, prompt, and plugin options not yet covered by structured settings here.", "结构化设置尚未覆盖的工具、显示、提示词和插件选项可在此修改。")}</div>
      {parseError && <div className="settings-inline-error">{parseError}</div>}
      <JsonCodeEditor value={raw} onChange={handleChange} height="calc(100vh - 230px)" ariaLabel={t("Complete AppConfig JSON", "完整 AppConfig JSON")} />
    </section>
  );
}
