const SECRET_KEYS = ["api_key", "token", "secret", "password", "webhook"];
import { PasswordField } from "../../shared/ui/password-field";
import { useI18n } from "../i18n/use-i18n";

type StructuredConfigFieldsProps = {
  value: Record<string, unknown>;
  onChange: (value: Record<string, unknown>) => void;
};

/**
 * 渲染由基础类型和数组组成的结构化配置字段。
 *
 * @param props 配置对象和更新回调
 * @returns 可编辑字段集合
 */
export function StructuredConfigFields({ value, onChange }: StructuredConfigFieldsProps) {
  const { t } = useI18n();
  const entries = Object.entries(value);
  if (entries.length === 0) return <div className="settings-state">{t("This configuration group has no editable fields", "当前配置组没有可编辑字段")}</div>;
  return (
    <div className="settings-form-grid structured-config-grid">
      {entries.map(([key, field]) => (
        <StructuredField
          key={key}
          name={key}
          value={field}
          onChange={(next) => onChange({ ...value, [key]: next })}
        />
      ))}
    </div>
  );
}

/**
 * 按字段值类型选择输入控件。
 *
 * @param props 字段名称、字段值和更新回调
 * @returns 单个配置字段
 */
function StructuredField({ name, value, onChange }: { name: string; value: unknown; onChange: (value: unknown) => void }) {
  const { t } = useI18n();
  const label = fieldLabel(name, t);
  if (typeof value === "boolean") {
    return (
      <label className="settings-toggle-field">
        <span><strong>{label}</strong><small>{name}</small></span>
        <input type="checkbox" checked={value} onChange={(event) => onChange(event.target.checked)} />
      </label>
    );
  }
  if (typeof value === "number") {
    return <label className="settings-field"><span>{label}</span><input type="number" value={value} onChange={(event) => onChange(Number(event.target.value))} /><small>{name}</small></label>;
  }
  if (Array.isArray(value)) {
    return (
      <label className="settings-field full">
        <span>{label}</span>
        <textarea rows={Math.min(8, Math.max(3, value.length + 1))} value={value.join("\n")} onChange={(event) => onChange(event.target.value.split("\n").map((item) => item.trim()).filter(Boolean))} spellCheck={false} />
        <small>{t(`${name}, one item per line`, `${name}，每行一项`)}</small>
      </label>
    );
  }
  if (value && typeof value === "object") {
    return (
      <fieldset className="settings-nested-field full">
        <legend>{label}</legend>
        <StructuredConfigFields value={value as Record<string, unknown>} onChange={onChange as (value: Record<string, unknown>) => void} />
      </fieldset>
    );
  }
  const secret = SECRET_KEYS.some((key) => name.toLowerCase().includes(key));
  if (secret) return <div className="settings-field"><span>{label}</span><PasswordField value={String(value ?? "")} onChange={onChange} /><small>{name}</small></div>;
  return <label className="settings-field"><span>{label}</span><input type="text" value={String(value ?? "")} onChange={(event) => onChange(event.target.value)} spellCheck={false} autoComplete="off" /><small>{name}</small></label>;
}

/**
 * 将配置字段标识转换为可读标签。
 *
 * @param name 配置字段标识
 * @param t 双语文本选择方法
 * @returns 可读字段名称
 */
function fieldLabel(name: string, t: (en: string, zh: string) => string): string {
  const labels: Record<string, string> = {
    enabled: t("Enabled", "启用"),
    max_rounds: t("Maximum tool rounds", "最大工具轮次"),
    command_shell: t("Command Shell", "命令 Shell"),
    progressive_loading_enabled: t("Progressive loading", "渐进式加载"),
    background_commands_enabled: t("Allow background commands", "允许后台命令"),
    background_command_timeout_seconds: t("Background command timeout", "后台命令超时"),
    background_command_log_max_bytes: t("Background log limit", "后台日志上限"),
    background_command_stop_grace_seconds: t("Stop grace period", "停止宽限时间"),
    allow_command_execution: t("Allow skills to run commands", "允许技能执行命令"),
    reasoning: t("Reasoning display", "思考显示方式"),
    tool_calls: t("Tool call display", "工具显示方式"),
    readable_tool_names: t("Readable tool names", "可读工具名称"),
    wait_show_model: t("Show model while waiting", "等待时显示模型"),
    wait_show_thinking_level: t("Show thinking level while waiting", "等待时显示思考等级"),
    repl_transcript_row_cap: t("Terminal transcript row limit", "终端记录行数上限"),
    default_max_chars: t("Default context tokens", "默认上下文 token 数")
  };
  return labels[name] ?? name.replaceAll("_", " ");
}
