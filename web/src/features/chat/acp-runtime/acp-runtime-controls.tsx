import { SlidersHorizontal } from "lucide-react";
import { useState } from "react";
import type { EngineStatusResponse } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { Modal } from "../../../shared/ui/dialog/modal";
import { Select } from "../../../shared/ui/select/select";
import { useI18n } from "../../i18n/use-i18n";
import { acpAdjustableOptions, type AcpOptionValue, type AcpRuntimeOption } from "./acp-runtime-options";
import { useAcpRuntimeConfig } from "./use-acp-runtime-config";
import "./acp-runtime-controls.css";

type AcpRuntimeControlsProps = {
  /** 当前外部内核运行状态 */
  status: EngineStatusResponse;
  /** 本轮对话是否进行中 */
  running: boolean;
};

/**
 * 渲染主页面的外部内核运行参数入口。
 *
 * 模型与思考等级已有专用选择器，这里承担权限模式与 agent 自报的其余配置项，
 * 让外部内核的调整方式与内置内核保持一致，不再散落在设置页。
 *
 * @param props 内核状态与运行标记
 * @returns 运行参数弹层入口
 */
export function AcpRuntimeControls({ status, running }: AcpRuntimeControlsProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const runtime = useAcpRuntimeConfig(open);
  const options = acpAdjustableOptions(status);
  // agent 尚未上报任何可调项时不展示入口，避免给出一个空弹层
  if (options.length === 0) return null;

  return (
    <>
      <Button
        className="acp-runtime-trigger"
        onClick={() => setOpen(true)}
        disabled={running}
        title={t(
          `Adjust ${status.label} runtime options`,
          `调整 ${status.label} 运行参数`
        )}
        aria-label={t("Engine runtime options", "内核运行参数")}
      >
        <SlidersHorizontal size={14} aria-hidden />
      </Button>
      <Modal
        open={open}
        title={t(`${status.label} runtime options`, `${status.label} 运行参数`)}
        description={t(
          "Values the engine reports for this session. Changes save immediately and apply to Web, TUI, and CLI runs.",
          "内核为当前会话公布的可调参数。修改即时保存，并应用到 Web、TUI 与 CLI 运行。"
        )}
        size="small"
        onClose={() => setOpen(false)}
      >
        {runtime.loading && (
          <div className="acp-runtime-state">{t("Loading engine options", "正在读取内核参数")}</div>
        )}
        {!runtime.loading && Boolean(runtime.error) && (
          <div className="acp-runtime-error">{errorMessage(runtime.error)}</div>
        )}
        {!runtime.loading && !runtime.error && (
          <div className="acp-runtime-grid">
            {options.map((option) => (
              <AcpOptionField
                key={option.id}
                option={option}
                value={resolveValue(option, runtime.acp)}
                disabled={runtime.saving}
                onChange={(value) => commit(option, value, runtime)}
              />
            ))}
          </div>
        )}
      </Modal>
    </>
  );
}

/**
 * 提取可展示的错误文本。
 *
 * @param error 查询或保存抛出的错误
 * @returns 错误消息
 */
function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * 读取配置项当前生效值。
 *
 * 已保存的覆盖值优先；未覆盖时使用 agent 上报的当前值。
 *
 * @param option 配置项定义
 * @param acp 已保存的 ACP 配置片段
 * @returns 当前生效值
 */
function resolveValue(option: AcpRuntimeOption, acp: Record<string, unknown>): AcpOptionValue {
  const stored = option.category === "mode"
    ? acp.permission_mode
    : (acp.config_options as Record<string, unknown> | undefined)?.[option.id];
  if (typeof stored === "string" || typeof stored === "boolean") return stored;
  return option.currentValue;
}

/**
 * 写入配置项新值。
 *
 * 权限模式落在 ACP 的专用字段上，其余项写进 config_options。
 *
 * @param option 配置项定义
 * @param value 新值
 * @param runtime 配置写入入口
 * @returns 无返回值
 */
function commit(
  option: AcpRuntimeOption,
  value: AcpOptionValue,
  runtime: ReturnType<typeof useAcpRuntimeConfig>
): void {
  if (option.category === "mode") {
    runtime.saveField("permission_mode", value);
    return;
  }
  runtime.saveOption(option.id, value);
}

/**
 * 渲染单个 ACP 配置项。
 *
 * @param props 配置定义、当前值、禁用状态与更新回调
 * @returns 选择框或开关
 */
function AcpOptionField({
  option,
  value,
  disabled,
  onChange
}: {
  option: AcpRuntimeOption;
  value: AcpOptionValue;
  disabled: boolean;
  onChange: (value: AcpOptionValue) => void;
}) {
  if (option.type === "boolean") {
    return (
      <label className="acp-runtime-toggle">
        <span>
          <strong>{option.name}</strong>
          <small>{option.description || option.id}</small>
        </span>
        <input
          type="checkbox"
          checked={value === true}
          disabled={disabled}
          onChange={(event) => onChange(event.target.checked)}
        />
      </label>
    );
  }
  return (
    <div className="acp-runtime-field">
      <span>{option.name}</span>
      <Select
        value={typeof value === "string" ? value : String(option.currentValue)}
        options={option.values}
        disabled={disabled}
        onChange={onChange}
        ariaLabel={option.name}
        menuPreferredWidth={280}
      />
      {option.description && <small>{option.description}</small>}
    </div>
  );
}
