import { useEffect, useState } from "react";
import { useI18n } from "../../features/i18n/use-i18n";

type ModelIconProps = {
  model: string;
  provider?: string | null;
  size?: number;
};

const PROVIDERS: Array<[RegExp, string]> = [
  [/gpt|^o[0-9]|codex|openai/i, "openai"],
  [/claude|opus|sonnet|haiku|anthropic/i, "anthropic"],
  [/gemini|palm|google/i, "google"],
  [/deepseek/i, "deepseek"],
  [/qwen|qwq|alibaba|dashscope/i, "alibaba"],
  [/glm|zhipu|chatglm/i, "zhipuai"],
  [/llama|meta-llama|meta\//i, "meta"],
  [/mistral|mixtral|codestral/i, "mistral"],
  [/grok|xai/i, "xai"],
  [/kimi|moonshot/i, "moonshotai"],
  [/yi-|01-ai|lingyi/i, "01-ai"],
  [/command-r|cohere/i, "cohere"],
  [/perplexity|sonar/i, "perplexity"],
  [/groq/i, "groq"],
  [/together/i, "togetherai"],
  [/fireworks/i, "fireworks-ai"],
  [/minimax/i, "minimax"],
  [/doubao|volcengine|byte/i, "bytedance"],
  [/hunyuan|tencent/i, "tencent"],
  [/ernie|baidu/i, "baidu"],
  [/step-|stepfun/i, "stepfun"],
  [/siliconflow/i, "siliconflow"]
];

/**
 * 使用 models.dev 供应商 SVG 渲染模型图标，失败时显示文字回退。
 *
 * @param props 模型标识、可选供应商标识和尺寸
 * @returns 模型图标
 */
export function ModelIcon({ model, provider, size = 16 }: ModelIconProps) {
  const { t } = useI18n();
  // 1. 目录返回的供应商标识
  // 2. 模型名正则推断
  // 3. provider/model 前缀
  const candidates = [
    normalizeProvider(provider),
    PROVIDERS.find(([pattern]) => pattern.test(model))?.[1],
    providerFromModelId(model)
  ].filter((item, index, list): item is string => Boolean(item) && list.indexOf(item) === index);
  const candidateKey = candidates.join("|");
  const [failedProviders, setFailedProviders] = useState<string[]>([]);
  // 模型或候选供应商变化时清空失败记录，避免跨模型污染
  useEffect(() => {
    setFailedProviders([]);
  }, [model, candidateKey]);
  const resolvedProvider = candidates.find((item) => !failedProviders.includes(item));

  if (resolvedProvider) {
    return (
      <img
        width={size}
        height={size}
        src={`https://models.dev/logos/${resolvedProvider}.svg`}
        alt=""
        aria-hidden="true"
        onError={() => setFailedProviders((current) => current.includes(resolvedProvider) ? current : [...current, resolvedProvider])}
        style={{ objectFit: "contain" }}
      />
    );
  }

  return (
    <span
      aria-label={t(`Model ${model}`, `模型 ${model}`)}
      style={{
        display: "inline-grid",
        width: size,
        height: size,
        placeItems: "center",
        borderRadius: 4,
        background: "var(--graphite-soft,#394244)",
        color: "#eef2f0",
        fontSize: Math.max(8, size * 0.48),
        fontWeight: 600
      }}
    >
      {model.slice(0, 2)}
    </span>
  );
}

/**
 * 规范化目录返回的供应商标识，便于映射 models.dev logo。
 *
 * @param provider 原始供应商标识
 * @returns 可用于 logo 路径的标识
 */
function normalizeProvider(provider?: string | null): string | undefined {
  if (!provider) return undefined;
  const value = provider.trim().toLowerCase();
  if (!value) return undefined;
  const aliases: Record<string, string> = {
    "openai-compatible": "openai",
    "azure-openai": "openai",
    "azure_ai": "openai",
    "vertex_ai": "google",
    "vertex-ai": "google",
    "google-vertex": "google",
    "google_ai_studio": "google",
    "gemini": "google",
    "anthropic-messages": "anthropic",
    "meta-llama": "meta",
    "x-ai": "xai",
    "zhipu": "zhipuai",
    "dashscope": "alibaba",
    "qwen": "alibaba",
    "alibaba-token-plan": "alibaba",
    "alibaba-token-plan-cn": "alibaba",
    "alibaba-coding-plan": "alibaba",
    "alibaba-coding-plan-cn": "alibaba",
    "alibaba-cn": "alibaba",
    "moonshot": "moonshotai",
    "together_ai": "togetherai",
    "fireworks_ai": "fireworks-ai",
    "byteplus": "bytedance",
    "volcengine": "bytedance",
    "nano-gpt": "openai",
    "llmgateway": "openai",
    "helicone": "openai"
  };
  return aliases[value] || value.replace(/[_\s]+/g, "-");
}

/**
 * 从 `provider/model` 形式的模型 ID 提取供应商前缀。
 *
 * @param model 模型标识
 * @returns 供应商标识
 */
function providerFromModelId(model: string): string | undefined {
  const slash = model.indexOf("/");
  if (slash <= 0) return undefined;
  return normalizeProvider(model.slice(0, slash));
}
