import { StructuredConfigFields } from "../structured-config-fields";
import {
  groupCliToolFields,
  isCliToolEnabledField,
  type CliToolFieldGroupId
} from "./cli-tool-field-groups";
import { useI18n } from "../../i18n/use-i18n";

type CliToolConfigEditorProps = {
  config: Record<string, unknown>;
  secretSentinel: string;
  onChange: (config: Record<string, unknown>) => void;
};

/**
 * 渲染单个 CLI 助手工具的结构化配置。
 *
 * @param props 工具配置、敏感字段占位符和更新回调
 * @returns 工具总开关与分组配置表单
 */
export function CliToolConfigEditor({
  config,
  secretSentinel,
  onChange
}: CliToolConfigEditorProps) {
  const { t } = useI18n();
  const enabledKey = Object.keys(config).find(isCliToolEnabledField);
  const enabled = enabledKey ? config[enabledKey] !== false : true;
  const groups = groupCliToolFields(config);

  const groupTitles: Record<CliToolFieldGroupId, string> = {
    credentials: t("Credentials", "凭据"),
    endpoints: t("Endpoints and paths", "服务地址与路径"),
    limits: t("Limits and timeouts", "限额与超时"),
    switches: t("Behavior switches", "行为开关"),
    other: t("Other options", "其他选项")
  };
  const groupHints: Record<CliToolFieldGroupId, string> = {
    credentials: t("Sensitive values remain masked until replaced.", "敏感值在替换前保持隐藏。"),
    endpoints: t("Leave builtin service addresses unchanged unless a proxy is required.", "仅在需要代理时修改内置服务地址。"),
    limits: t("Control execution cost, output size, and latency.", "控制执行开销、输出大小和耗时。"),
    switches: t("Adjust optional behavior without changing tool availability.", "调整细节行为，不改变工具可用状态。"),
    other: t("Tool-specific runtime parameters.", "该工具专用的运行参数。")
  };

  return (
    <div className={enabled ? "cli-tool-config-editor" : "cli-tool-config-editor is-disabled"}>
      {enabledKey && (
        <label className="settings-toggle-field cli-tool-enable-row">
          <span>
            <strong>{t("Available to CLI assistants", "对 CLI 助手开放")}</strong>
            <small>
              {enabled
                ? t("The assistant may select this tool when the task requires it.", "CLI 助手可在任务需要时选择该工具。")
                : t("The tool stays configured but is not exposed to CLI assistants.", "配置继续保留，但不会向 CLI 助手开放。")}
            </small>
          </span>
          <input
            type="checkbox"
            checked={enabled}
            aria-label={t("Available to CLI assistants", "对 CLI 助手开放")}
            onChange={(event) => onChange({ ...config, [enabledKey]: event.target.checked })}
          />
        </label>
      )}
      {groups.length === 0 ? (
        <div className="settings-state cli-tool-empty-options">
          {t("This tool has no additional options", "该工具没有其他可配置项")}
        </div>
      ) : (
        <div className="cli-tool-config-groups">
          {groups.map((group) => (
            <section className="cli-tool-config-group" key={group.id}>
              <header className="cli-tool-config-group-head">
                <h3>{groupTitles[group.id]}</h3>
                <p>{groupHints[group.id]}</p>
              </header>
              <StructuredConfigFields
                value={Object.fromEntries(group.entries)}
                secretSentinel={secretSentinel}
                onChange={(next) => onChange({ ...config, ...next })}
              />
            </section>
          ))}
        </div>
      )}
    </div>
  );
}
