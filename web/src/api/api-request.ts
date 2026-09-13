import { ApiError } from "./api-error";

/**
 * 【网页接口】【请求】发送 JSON API 请求并统一处理错误。
 * @param path 同源接口路径
 * @param init 可选请求方法、正文和请求头
 * @returns 解析后的响应数据，失败时抛出接口错误
 */
export async function apiRequest<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    credentials: "same-origin",
    ...init,
    headers: {
      ...(init?.body ? { "Content-Type": "application/json" } : {}),
      ...init?.headers
    }
  });
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { error?: string; detail?: string } | null;
    const message = body?.error ?? `HTTP ${response.status}`;
    throw new ApiError(message, body?.detail ?? message);
  }
  return response.json() as Promise<T>;
}
