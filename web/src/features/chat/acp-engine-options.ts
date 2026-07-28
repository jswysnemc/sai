import type { AppConfig, EngineStatusResponse, ThinkingLevel } from "../../api/contracts";
import type { ChatModelChoice } from "./chat-model-options";

type AcpConfigOption = {
  category?: unknown;
  currentValue?: unknown;
  options?: unknown;
};

export const ACP_PROVIDER_ID = "__acp__";

const THINKING_LEVELS: ThinkingLevel[] = ["auto", "none", "low", "medium", "high", "xhigh", "max"];

/**
 * 【ACP】【模型能力】从 ACP 运行状态构造外部内核模型选项。
 *
 * @param status 当前内核运行状态
 * @param config 应用配置，用于握手前的回退值
 * @returns 外部内核模型选项
 */
export function buildAcpModelChoices(
  status: EngineStatusResponse | undefined,
  config: AppConfig
): ChatModelChoice[] {
  const advertised = advertisedAcpModelChoices(status);
  if (advertised.length > 0) return advertised;
  const configured = config.agent?.acp?.model?.trim();
  return configured
    ? [{ providerId: ACP_PROVIDER_ID, providerName: status?.label ?? "ACP", model: configured }]
    : [];
}

/**
 * 【ACP】【模型能力】返回当前运行时明确公布的模型选项。
 *
 * @param status 当前内核运行状态
 * @returns ACP 运行时公布的模型选项；未公布时返回空数组
 */
export function advertisedAcpModelChoices(
  status: EngineStatusResponse | undefined
): ChatModelChoice[] {
  const modelOption = configOptions(status).find((option) => option.category === "model");
  const values = selectValues(modelOption?.options);
  return values.map((option) => ({
    providerId: ACP_PROVIDER_ID,
    providerName: status?.label ?? "ACP",
    model: option.value
  }));
}

/**
 * 【ACP】【模型能力】返回 ACP 当前模型值。
 *
 * @param status 当前内核运行状态
 * @returns agent 公布的当前模型值
 */
export function currentAcpModel(status: EngineStatusResponse | undefined): string | undefined {
  const option = configOptions(status).find((item) => item.category === "model");
  return typeof option?.currentValue === "string" ? option.currentValue : undefined;
}

/**
 * 【ACP】【思考能力】返回 ACP 内核实际支持的思考等级。
 *
 * @param status 当前内核运行状态
 * @returns Sai 界面能够表示的思考等级
 */
export function acpThinkingLevels(status: EngineStatusResponse | undefined): ThinkingLevel[] {
  const option = configOptions(status).find((item) => item.category === "thought_level");
  const supported = new Set(selectValues(option?.options).map((item) => item.value));
  return THINKING_LEVELS.filter((level) => supported.has(level));
}

/**
 * 读取运行状态中的标准配置项。
 *
 * @param status 当前内核运行状态
 * @returns 可安全读取的配置项数组
 */
function configOptions(status: EngineStatusResponse | undefined): AcpConfigOption[] {
  const options = status?.acp_runtime?.config_options;
  return Array.isArray(options) ? options as AcpConfigOption[] : [];
}

/**
 * 展开 ACP 扁平或分组的选择项。
 *
 * @param options 协议 options 字段
 * @returns 已过滤的值与展示名称
 */
function selectValues(options: unknown): Array<{ value: string; name: string }> {
  if (!Array.isArray(options)) return [];
  const flattened = options.flatMap((item) => {
    const nested = (item as { options?: unknown }).options;
    return Array.isArray(nested) ? nested : [item];
  });
  return flattened.flatMap((item) => {
    const option = item as { value?: unknown; name?: unknown };
    if (typeof option.value !== "string") return [];
    return [{
      value: option.value,
      name: typeof option.name === "string" ? option.name : option.value
    }];
  });
}
