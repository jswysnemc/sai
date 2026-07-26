import { StructuredConfigFields } from "../structured-config-fields";
import { groupPluginFields, isPluginEnabledField, type PluginFieldGroupId } from "./plugin-field-groups";
import { useI18n } from "../../i18n/use-i18n";
import "./plugin-config-editor.css";

type PluginConfigEditorProps = {
  /** 当前插件的配置对象 */
  config: Record<string, unknown>;
  /** 配置更新回调 */
  onChange: (config: Record<string, unknown>) => void;
};

/**
 * 渲染单个插件的配置表单。
 *
 * 排版分三层：总开关独占顶部，其余字段按语义分组，组内才是原有的字段网格。
 * 关闭状态下后续分组降低不透明度但仍可编辑——预先填好凭据再打开是常见操作。
 *
 * @param props 插件配置与更新回调
 * @returns 插件配置表单
 */
export function PluginConfigEditor({ config, onChange }: PluginConfigEditorProps) {
  const { t } = useI18n();
  const enabledKey = Object.keys(config).find(isPluginEnabledField);
  const enabled = enabledKey ? config[enabledKey] !== false : true;
  const groups = groupPluginFields(config);

  const groupTitles: Record<PluginFieldGroupId, string> = {
    credentials: t("Credentials", "凭据"),
    endpoints: t("Endpoints and paths", "服务地址与路径"),
    limits: t("Limits and timeouts", "限额与超时"),
    switches: t("Behavior switches", "行为开关"),
    other: t("Other options", "其他选项")
  };

  const groupHints: Record<PluginFieldGroupId, string> = {
    credentials: t("Stored locally and masked in the interface.", "保存在本地，界面中以掩码显示。"),
    endpoints: t("Leave empty to use the builtin defaults.", "留空则使用内置默认值。"),
    limits: t("Guards against runaway cost and latency.", "用于约束开销与耗时。"),
    switches: t("Fine-grained behavior of this plugin.", "该插件的细节行为。"),
    other: t("Fields without a dedicated group.", "没有归入上述分组的字段。")
  };

  return (
    <div className="plugin-config-editor">
      {enabledKey && (
        <div className={enabled ? "plugin-enable-row is-on" : "plugin-enable-row"}>
          <span className="plugin-enable-copy">
            <strong>{t("Enable plugin", "启用插件")}</strong>
            <small>
              {enabled
                ? t("Tools of this plugin are exposed to the model.", "该插件的工具会暴露给模型。")
                : t("Tools stay hidden; settings below are still editable.", "工具不会暴露；下方配置仍可编辑。")}
            </small>
          </span>
          <input
            type="checkbox"
            checked={enabled}
            aria-label={t("Enable plugin", "启用插件")}
            onChange={(event) => onChange({ ...config, [enabledKey]: event.target.checked })}
          />
        </div>
      )}
      {groups.length === 0 ? (
        <div className="settings-state">
          {t("This plugin has no additional options", "该插件没有其他可配置项")}
        </div>
      ) : (
        groups.map((group) => (
          <section className="plugin-config-group" key={group.id}>
            <header className="plugin-config-group-head">
              <h4>{groupTitles[group.id]}</h4>
              <small>{groupHints[group.id]}</small>
            </header>
            <StructuredConfigFields
              value={Object.fromEntries(group.entries)}
              onChange={(next) => onChange({ ...config, ...next })}
            />
          </section>
        ))
      )}
    </div>
  );
}
