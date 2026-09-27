import { apiRequest } from "./api-request";
import type { ImageEndpointProbeReport, ModelEndpointConfig } from "./contracts";
import type { JevProbeReport, JevStatus } from "./contracts/jev";

/** 生图模型接入：目录、探测与直接生成。 */
export const imageModelsApi = {
  /** 获取当前生图端点公开的模型目录。 */
  models: (endpoint: ModelEndpointConfig) =>
    apiRequest<{ models: string[] }>("/api/image-models/models", {
      method: "POST",
      body: JSON.stringify({ endpoint })
    }),
  /** 用一次最小真实图片请求探测生图端点。 */
  test: (endpoint: ModelEndpointConfig) =>
    apiRequest<ImageEndpointProbeReport>("/api/image-models/test", {
      method: "POST",
      body: JSON.stringify({ endpoint })
    }),
  /** 根据聊天中的提示词直接请求图片模型。 */
  generate: (request: { endpointId: string; model?: string; prompt: string; aspectRatio: string; resolution: string; images?: string[] }) =>
    apiRequest<Record<string, unknown>>("/api/image-models/generate", {
      method: "POST",
      body: JSON.stringify({
        endpoint_id: request.endpointId,
        model: request.model,
        prompt: request.prompt,
        aspect_ratio: request.aspectRatio,
        resolution: request.resolution,
        images: request.images ?? []
      })
    })
};

/** 内置 Jev：状态与连接测试。 */
export const jevApi = {
  /** 读取已保存配置下的接入、密钥与功能状态。 */
  status: () => apiRequest<JevStatus>("/api/jev/status"),
  /** 测试接入；传入草稿时测试草稿，省略时测试已保存的生效接入。 */
  test: (endpoint?: ModelEndpointConfig) =>
    apiRequest<JevProbeReport>("/api/jev/test", {
      method: "POST",
      body: JSON.stringify({ endpoint: endpoint ?? null })
    })
};
