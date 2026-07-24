import type { AppConfig } from "../../../api/contracts";
import { Link } from "react-router-dom";
import { SettingsGroup } from "../editor-layout";
import { StructuredConfigFields } from "../structured-config-fields";
import { useI18n } from "../../i18n/use-i18n";

type SkillBehaviorSettingsProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
  /** 是否展示指向 Skills 文档管理的说明链接。 */
  showLibraryLink?: boolean;
};

/**
 * 渲染 AppConfig.skills 行为字段（加载与命令执行权限）。
 *
 * @param props 应用配置与更新回调
 * @returns 技能行为设置分组
 */
export function SkillBehaviorSettings({
  config,
  onConfigChange,
  showLibraryLink = false
}: SkillBehaviorSettingsProps) {
  const { t } = useI18n();
  const value = (config.skills as Record<string, unknown> | undefined) ?? {};
  return (
    <SettingsGroup
      title={t("Skill behavior", "技能行为")}
      description={t(
        "Control progressive loading and whether Skills may run shell commands. Skill files are managed separately.",
        "控制渐进式加载以及 Skills 是否可执行 shell 命令。Skill 文档在独立区域管理。"
      )}
    >
      {showLibraryLink && (
        <p className="settings-inline-note">
          {t("Manage Skill documents in", "Skill 文档管理见")}{" "}
          <Link to="/settings/skills">{t("Skills library", "Skills 库")}</Link>
          {t(". Save behavior changes with the top Save button.", "。行为字段请用顶栏保存。")}
        </p>
      )}
      {!showLibraryLink && (
        <p className="settings-inline-note">
          {t(
            "These fields belong to AppConfig. Use the top Save button after editing. File create/edit uses the editor save control.",
            "以下字段属于 AppConfig，修改后请用顶栏保存；文档新建/编辑使用编辑器内保存。"
          )}
        </p>
      )}
      <StructuredConfigFields
        value={value}
        onChange={(next) => onConfigChange({ ...config, skills: next })}
      />
    </SettingsGroup>
  );
}
