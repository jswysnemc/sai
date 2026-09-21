import { useI18n } from "../../i18n/use-i18n";

/** 返回生图请求格式选项。 */
export function imageProtocolOptions() {
  const { t } = useI18n();
  return [
    { value: "auto", label: t("Auto detect", "自动检测"), description: t("Infer from the endpoint and model name", "根据地址与模型名判断") },
    { value: "openai-images", label: "OpenAI Images", description: t("/images/generations JSON", "/images/generations JSON 请求") },
    { value: "gemini", label: "Google Gemini", description: t("generateContent with inline image data", "generateContent 与内嵌图片数据") }
  ];
}
