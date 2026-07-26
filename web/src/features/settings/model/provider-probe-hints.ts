import type { ProviderProbeErrorKind } from "../../../api/contracts";

type Translate = (en: string, zh: string) => string;

/**
 * 按失败类型给出可操作的修复建议。
 *
 * 只报"连接失败"没有诊断价值，这里把每类失败落到具体该改哪一项配置。
 *
 * @param kind 失败归类
 * @param t 双语取值函数
 * @returns 修复建议；未归类时返回空串
 */
export function probeHint(kind: ProviderProbeErrorKind | undefined, t: Translate): string {
  switch (kind) {
    case "network":
      return t(
        "Cannot reach the endpoint. Check the base URL, DNS, and any proxy settings.",
        "无法连接该地址。请检查 Base URL、DNS 与代理设置。"
      );
    case "timeout":
      return t(
        "The request timed out. The endpoint may be slow or blocked by a firewall.",
        "请求超时。该地址可能响应过慢，或被防火墙拦截。"
      );
    case "auth":
      return t(
        "Credentials were rejected. Check the API key and whether it has access to this endpoint.",
        "凭据被拒绝。请检查 API Key，以及该密钥是否有此端点的权限。"
      );
    case "not_found":
      return t(
        "The endpoint or model identifier does not exist. Check the base URL path and the model name.",
        "端点或模型标识不存在。请检查 Base URL 路径与模型名称。"
      );
    case "rate_limit":
      return t(
        "Rate limited or out of quota. Retry later or switch to another key.",
        "触发限流或额度用尽。请稍后重试，或更换密钥。"
      );
    case "server":
      return t(
        "The upstream service returned an error. This is usually temporary.",
        "上游服务返回错误，通常是临时故障。"
      );
    case "protocol":
      return t(
        "The response could not be parsed. The base URL may point at a non-compatible endpoint.",
        "响应无法解析。Base URL 可能指向了不兼容的端点。"
      );
    default:
      return "";
  }
}

/**
 * 取探测阶段的显示名。
 *
 * @param stage 阶段标识
 * @param t 双语取值函数
 * @returns 阶段显示名
 */
export function stageLabel(stage: string, t: Translate): string {
  if (stage === "catalog") return t("Endpoint & credentials", "地址与凭据");
  if (stage === "completion") return t("Model response", "模型响应");
  return stage;
}
