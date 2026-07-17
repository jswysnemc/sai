import type { ThinkingLevel } from "../../api/contracts";

export type ThinkingOption = {
  value: ThinkingLevel;
  label: string;
  descriptionEn: string;
  descriptionZh: string;
};

export const THINKING_OPTIONS: ThinkingOption[] = [
  { value: "auto", label: "auto", descriptionEn: "Let the provider decide based on model capabilities", descriptionZh: "由服务商按模型能力决定" },
  { value: "none", label: "none", descriptionEn: "Do not request additional reasoning", descriptionZh: "不请求额外推理" },
  { value: "low", label: "low", descriptionEn: "Faster responses with less reasoning", descriptionZh: "更快响应，使用较少推理" },
  { value: "medium", label: "medium", descriptionEn: "Balance response speed and reasoning depth", descriptionZh: "平衡响应速度与推理深度" },
  { value: "high", label: "high", descriptionEn: "Suitable for complex implementation tasks", descriptionZh: "适合复杂实现任务" },
  { value: "xhigh", label: "xhigh", descriptionEn: "Increase the reasoning budget for complex problems", descriptionZh: "增加复杂问题推理预算" },
  { value: "max", label: "max", descriptionEn: "Use the highest level supported by the provider", descriptionZh: "使用服务商支持的最高等级" }
];

/**
 * 返回思考等级的展示名称（英文 token，如 high / xhigh）。
 *
 * @param value 当前思考等级
 * @returns 对应展示名称，未知值回退为 auto
 */
export function thinkingLevelLabel(value: ThinkingLevel): string {
  return THINKING_OPTIONS.find((option) => option.value === value)?.label ?? THINKING_OPTIONS[0].label;
}
