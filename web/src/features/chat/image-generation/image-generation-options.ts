import type { ModelEndpointConfig } from "../../../api/contracts";

export type ImageAspectRatio = "1:1" | "4:3" | "3:4" | "16:9" | "9:16";

export type ImageResolution =
  | "1024x1024"
  | "2048x2048"
  | "1536x1152"
  | "1152x1536"
  | "1536x864"
  | "864x1536";

export type ImageGenerationSettings = {
  aspectRatio: ImageAspectRatio;
  resolution: ImageResolution;
};

export const DEFAULT_IMAGE_GENERATION_SETTINGS: ImageGenerationSettings = {
  aspectRatio: "1:1",
  resolution: "1024x1024"
};

export const IMAGE_ASPECT_RATIOS: Array<{ value: ImageAspectRatio; label: string }> = [
  { value: "1:1", label: "1:1" },
  { value: "4:3", label: "4:3" },
  { value: "3:4", label: "3:4" },
  { value: "16:9", label: "16:9" },
  { value: "9:16", label: "9:16" }
];

export const IMAGE_RESOLUTIONS: Array<{ value: ImageResolution; label: string }> = [
  { value: "1024x1024", label: "1024 × 1024" },
  { value: "2048x2048", label: "2048 × 2048" },
  { value: "1536x1152", label: "1536 × 1152" },
  { value: "1152x1536", label: "1152 × 1536" },
  { value: "1536x864", label: "1536 × 864" },
  { value: "864x1536", label: "864 × 1536" }
];

/** 为每个画面比例提供匹配的标准尺寸，避免比例和分辨率互相矛盾。 */
const IMAGE_RESOLUTIONS_BY_ASPECT_RATIO: Record<ImageAspectRatio, ImageResolution[]> = {
  "1:1": ["1024x1024", "2048x2048"],
  "4:3": ["1536x1152"],
  "3:4": ["1152x1536"],
  "16:9": ["1536x864"],
  "9:16": ["864x1536"]
};

/** 返回指定比例下可用的分辨率选项。 */
export function imageResolutionsForAspectRatio(aspectRatio: ImageAspectRatio): Array<{ value: ImageResolution; label: string }> {
  const allowed = new Set(IMAGE_RESOLUTIONS_BY_ASPECT_RATIO[aspectRatio]);
  return IMAGE_RESOLUTIONS.filter((item) => allowed.has(item.value));
}

/** 返回配置中可用于聊天的生图端点。 */
export function imageGenerationEndpoints(config?: { model_endpoints?: ModelEndpointConfig[] }): ModelEndpointConfig[] {
  return (config?.model_endpoints ?? []).filter((endpoint) => endpoint.kind === "image_generation");
}

/** 从会话偏好和配置顺序中解析当前生图端点。 */
export function resolveImageEndpoint(endpoints: ModelEndpointConfig[], preferredId: string | null): ModelEndpointConfig | null {
  return endpoints.find((endpoint) => endpoint.id === preferredId) ?? endpoints[0] ?? null;
}

/** 读取当前会话保存的生图端点偏好。 */
export function readStoredImageEndpoint(sessionId?: string): string | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage.getItem(imageEndpointStorageKey(sessionId));
  } catch {
    return null;
  }
}

/** 保存当前会话的生图端点偏好。 */
export function writeStoredImageEndpoint(sessionId: string | undefined, endpointId: string): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(imageEndpointStorageKey(sessionId), endpointId);
  } catch {
    // 本地存储被禁用时仍允许当前会话使用选择
  }
}

function imageEndpointStorageKey(sessionId?: string): string {
  return sessionId ? `sai.image-endpoint.${sessionId}` : "sai.image-endpoint.global";
}
