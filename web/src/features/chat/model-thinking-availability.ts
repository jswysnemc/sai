import type { AppConfig, ThinkingLevel } from "../../api/contracts";
import type { ChatModelChoice } from "./chat-model-options";

/** 按强度升序排列的思考等级；auto 不在其列，它对任何模型都可用。 */
export const THINKING_LEVEL_ORDER: ThinkingLevel[] = ["none", "low", "medium", "high", "xhigh", "max"];

/**
 * 解析当前模型支持的思考等级。
 *
 * 返回 undefined 表示未知：模型目录没覆盖到的模型不该被锁死可选项，
 * 调用方按全部可用处理。
 *
 * @param config Sai 应用配置
 * @param selection 当前选中的模型
 * @returns 支持的等级列表；未记录时返回 undefined
 */
export function modelThinkingLevels(
  config: AppConfig | undefined,
  selection: ChatModelChoice | null
): ThinkingLevel[] | undefined {
  if (!config || !selection) return undefined;
  const provider = config.providers.find((item) => item.id === selection.providerId);
  const levels = provider?.model_metadata?.[selection.model]?.thinking_levels;
  if (!levels?.length) return undefined;
  const known = levels.filter((level): level is ThinkingLevel => (
    THINKING_LEVEL_ORDER.includes(level as ThinkingLevel)
  ));
  if (!known.length) return undefined;
  // auto 始终附在最前：它表示不发送思考参数，与模型支持哪些档位无关
  return ["auto", ...THINKING_LEVEL_ORDER.filter((level) => known.includes(level))];
}

/**
 * 把请求的等级落到可用档位上。
 *
 * 与后端 resolve_thinking_level 同一套就近降级规则：优先取不超过请求强度的
 * 最强档，都比它强时取最弱档。回退到 auto 会把"我要重思考"悄悄变成"随便"。
 *
 * @param available 可用等级；undefined 表示未知
 * @param level 请求的等级
 * @returns 落到可用档位后的等级
 */
export function resolveThinkingLevel(
  available: ThinkingLevel[] | undefined,
  level: ThinkingLevel
): ThinkingLevel {
  if (!available?.length || level === "auto" || available.includes(level)) return level;
  const requested = THINKING_LEVEL_ORDER.indexOf(level);
  if (requested < 0) return level;
  const ranked = available
    .filter((item) => item !== "auto")
    .map((item) => ({ item, rank: THINKING_LEVEL_ORDER.indexOf(item) }))
    .filter((entry) => entry.rank >= 0)
    .sort((left, right) => left.rank - right.rank);
  if (!ranked.length) return level;
  const below = ranked.filter((entry) => entry.rank <= requested).at(-1);
  return (below ?? ranked[0]).item;
}
