import { PasswordField } from "../../shared/ui/password-field";
import { type SelectOption } from "../../shared/ui/select/select";
import { NumberFieldRow, SelectFieldRow, TextFieldRow } from "./controls/field-row";
import { ToggleRow } from "./controls/toggle-row";
import { mergeSecretValues } from "./controls/merge-secret-values";
import { useI18n } from "../i18n/use-i18n";

const SECRET_KEYS = ["api_key", "token", "secret", "password", "webhook"];

type StructuredConfigFieldsProps = {
  value: Record<string, unknown>;
  /** 服务端用于表示敏感字段未修改的占位符 */
  secretSentinel?: string;
  onChange: (value: Record<string, unknown>) => void;
};

/**
 * 渲染由基础类型和数组组成的结构化配置字段。
 *
 * @param props 配置对象和更新回调
 * @returns 可编辑字段集合
 */
export function StructuredConfigFields({ value, secretSentinel = "", onChange }: StructuredConfigFieldsProps) {
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
          secretSentinel={secretSentinel}
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
function StructuredField({
  name,
  value,
  secretSentinel,
  onChange
}: {
  name: string;
  value: unknown;
  secretSentinel: string;
  onChange: (value: unknown) => void;
}) {
  const { t } = useI18n();
  const label = fieldLabel(name, t);
  if (typeof value === "boolean") {
    return <ToggleRow label={label} hint={name} checked={value} onChange={onChange} />;
  }
  if (typeof value === "number") {
    return <NumberFieldRow label={label} hint={name} value={value} onChange={onChange} />;
  }
  const selectOptions = typeof value === "string" ? fieldSelectOptions(name, t) : null;
  if (selectOptions) {
    return (
      <SelectFieldRow
        label={label}
        hint={name}
        value={String(value)}
        options={selectOptions}
        onChange={onChange}
      />
    );
  }
  if (Array.isArray(value)) {
    const secret = isSecretField(name);
    const currentValues = value.map((item) => String(item ?? ""));
    const hiddenCount = secretSentinel && secret
      ? currentValues.filter((item) => item === secretSentinel).length
      : 0;
    const visibleValues = secretSentinel && secret
      ? currentValues.filter((item) => item !== secretSentinel && item.trim())
      : currentValues;
    return (
      <label className="settings-field full">
        <span>{label}</span>
        <textarea
          rows={Math.min(8, Math.max(3, visibleValues.length + 1))}
          value={visibleValues.join("\n")}
          onChange={(event) => {
            const nextVisible = event.target.value.split("\n").map((item) => item.trim()).filter(Boolean);
            onChange(secret && secretSentinel
              ? mergeSecretValues(currentValues, nextVisible, secretSentinel)
              : nextVisible);
          }}
          spellCheck={false}
        />
        <small>
          {hiddenCount > 0
            ? t(
                hiddenCount + " saved secret values remain unchanged; add one item per line.",
                "已保存 " + hiddenCount + " 个敏感值并保持不变；每行新增一项。"
              )
            : t(name + ", one item per line", name + "，每行一项")}
        </small>
      </label>
    );
  }
  if (value && typeof value === "object") {
    return (
      <fieldset className="settings-nested-field full">
        <legend>{label}</legend>
        <StructuredConfigFields
          value={value as Record<string, unknown>}
          secretSentinel={secretSentinel}
          onChange={onChange as (value: Record<string, unknown>) => void}
        />
      </fieldset>
    );
  }
  const secret = isSecretField(name);
  if (secret) {
    const hidden = Boolean(secretSentinel) && value === secretSentinel;
    return (
      <div className="settings-field">
        <span>{label}</span>
        <PasswordField
          value={hidden ? "" : String(value ?? "")}
          placeholder={hidden ? t("Saved value remains unchanged", "已保存的值保持不变") : undefined}
          onChange={onChange}
        />
        <small>{hidden ? t("A secret value is already saved.", "已保存敏感值。") : name}</small>
      </div>
    );
  }
  return <TextFieldRow label={label} hint={name} value={String(value ?? "")} onChange={onChange} />;
}

/**
 * 返回有限字符串配置对应的统一下拉选项。
 *
 * @param name 配置字段名称
 * @param t 双语文本选择方法
 * @returns 已知枚举的选项；普通字符串返回 null
 */
export function fieldSelectOptions(
  name: string,
  t: (en: string, zh: string) => string
): SelectOption<string>[] | null {
  if (name === "reasoning" || name === "tool_calls") {
    return [
      { value: "hidden", label: t("Hidden", "隐藏") },
      { value: "summary", label: t("Summary", "摘要") },
      { value: "full", label: t("Full", "完整") }
    ];
  }
  if (name === "command_filter") {
    return [
      { value: "auto", label: t("Automatic", "自动") },
      { value: "rtk", label: "rtk" },
      { value: "off", label: t("Disabled", "关闭") }
    ];
  }
  return null;
}

/**
 * 判断字段名称是否表示敏感值。
 *
 * @param name 配置字段名称
 * @returns 字段需要隐藏时返回 true
 */
function isSecretField(name: string): boolean {
  const normalized = name.toLowerCase();
  return SECRET_KEYS.some((key) => normalized.includes(key));
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
    api_key: t("API key", "接口密钥"),
    api_keys: t("API keys", "接口密钥"),
    base_url: t("Base URL", "服务地址"),
    output_dir: t("Output directory", "输出目录"),
    model: t("Model", "模型"),
    provider_type: t("Provider type", "供应商类型"),
    timeout_seconds: t("Timeout", "超时秒数"),
    max_results: t("Maximum results", "最大结果数量"),
    max_download_mb: t("Maximum download size", "最大下载大小"),
    safe_search: t("Safe search", "安全搜索"),
    auto_preview: t("Automatic preview", "自动预览"),
    preview_count: t("Preview count", "预览数量"),
    vision_screening_enabled: t("Vision screening", "视觉模型审核"),
    thinking_depth: t("Thinking depth", "思考深度"),
    max_review_revisions: t("Maximum review revisions", "最大审阅修正次数"),
    max_tool_steps_per_round: t("Tool steps per round", "每轮工具步数"),
    max_final_answer_chars: t("Final answer character limit", "最终答案字符上限"),
    tool_call_timeout_seconds: t("Tool call timeout", "工具调用超时"),
    max_tool_steps: t("Maximum tool steps", "最大工具步数"),
    show_progress: t("Show progress", "显示进度"),
    prefer_current_multimodal_model: t("Prefer current multimodal model", "优先使用当前多模态模型"),
    vision_provider_id: t("Vision provider", "视觉模型供应商"),
    vision_model: t("Vision model", "视觉模型"),
    preview_with_chafa: t("Preview with chafa", "使用 chafa 预览"),
    free_fallback_enabled: t("Free fallback", "免费回退"),
    default_aspect_ratio: t("Default aspect ratio", "默认宽高比"),
    default_resolution: t("Default resolution", "默认分辨率"),
    auto_print: t("Print after generation", "生成后显示"),
    width_percent: t("Width percentage", "宽度百分比"),
    height_percent: t("Height percentage", "高度百分比"),
    data_dir: t("Data directory", "数据目录"),
    embedding_enabled: t("Embedding retrieval", "启用向量检索"),
    embedding_provider_id: t("Embedding provider", "向量模型供应商"),
    embedding_model: t("Embedding model", "向量模型"),
    backend: t("Backend", "后端"),
    command_timeout_seconds: t("Command timeout", "命令超时"),
    max_stdout_chars: t("Standard output limit", "标准输出字符上限"),
    max_stderr_chars: t("Error output limit", "错误输出字符上限"),
    max_rounds: t("Maximum tool rounds", "最大工具轮次"),
    command_shell: t("Command Shell", "命令 Shell"),
    command_filter: t("Command output filter: auto / rtk / off", "命令输出过滤器：auto / rtk / off"),
    command_filter_denylist: t(
      "Commands kept out of rtk, added on top of the builtin list (cat/grep/ls...)",
      "不走 rtk 的命令，在内置列表（cat/grep/ls 等）之外追加",
    ),
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
  return labels[name] ?? humanizeFieldName(name);
}

/**
 * 将未知字段名称转换为统一的标题格式。
 *
 * @param name 配置字段名称
 * @returns 首字母大写并保留常见缩写的名称
 */
function humanizeFieldName(name: string): string {
  const acronyms: Record<string, string> = {
    api: "API",
    url: "URL",
    id: "ID",
    mb: "MB",
    gif: "GIF",
    json: "JSON"
  };
  return name
    .split("_")
    .map((word) => acronyms[word] ?? (word.charAt(0).toUpperCase() + word.slice(1)))
    .join(" ");
}
