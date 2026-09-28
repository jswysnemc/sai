import type { UsageGroupStats } from "../../../api/contracts";

/** 模型分布中的一项，可代表多个小模型合计。 */
export type ModelDistributionItem = Pick<UsageGroupStats, "id" | "label" | "total_tokens">;

/**
 * 【用量】【模型分布】将长尾模型合并为其他项，保证图表总量不减少。
 * @param rows 完整模型统计
 * @param otherLabel 其他项的本地化名称
 * @returns 最多六项的分布与完整总量
 */
export function buildModelDistribution(rows: readonly UsageGroupStats[], otherLabel: string) {
  const positive = rows.filter((row) => row.total_tokens > 0);
  const total = positive.reduce((sum, row) => sum + row.total_tokens, 0);
  const slices: ModelDistributionItem[] = positive.length <= 6 ? [...positive] : [
    ...positive.slice(0, 5),
    { id: "__remaining_models__", label: otherLabel, total_tokens: positive.slice(5).reduce((sum, row) => sum + row.total_tokens, 0) }
  ];
  return { slices, total };
}
