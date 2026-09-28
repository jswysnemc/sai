/** 「跟随当前模型」在选择器中的取值。 */
export const INHERIT_MODEL_VALUE = "";

/** 供应商与模型的分隔符，不会出现在合法的供应商标识或模型名中。 */
const CHOICE_SEPARATOR = "\u0000";

/**
 * 把供应商与模型编码为选择器取值。
 *
 * @param providerId 供应商标识
 * @param model 模型名称
 * @returns 选择器取值；任一为空时返回跟随当前模型
 */
export function encodeModelChoice(providerId: string | null | undefined, model: string | null | undefined): string {
  if (!providerId || !model) return INHERIT_MODEL_VALUE;
  return `${providerId}${CHOICE_SEPARATOR}${model}`;
}

/**
 * 把选择器取值解码为供应商与模型。
 *
 * @param value 选择器取值
 * @returns 供应商与模型；跟随当前模型时两者均为空串
 */
export function decodeModelChoice(value: string): { providerId: string; model: string } {
  if (!value) return { providerId: "", model: "" };
  const [providerId = "", model = ""] = value.split(CHOICE_SEPARATOR, 2);
  return { providerId, model };
}
