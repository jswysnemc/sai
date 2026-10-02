import { apiRequest } from "../../api/api-request";

export type TemplateScope = "chat" | "image";
export type InputTemplate = { name: string; content: string; builtin: boolean };
export const templateKey = (scope: TemplateScope) => ["input-templates", scope];

/** 构造模板路由；参数为场景与可选关键词，返回 API 地址。 */
function path(scope: TemplateScope, name?: string): string {
  return `/api/prompts/${scope}-templates${name === undefined ? "" : `/${encodeURIComponent(name)}`}`;
}

export const templateApi = {
  /** 读取当前场景全部模板；参数为场景，返回完整模板列表。 */
  async list(scope: TemplateScope): Promise<InputTemplate[]> {
    const result = await apiRequest<{ items: InputTemplate[] }>(path(scope));
    return result.items;
  },
  /** 保存自定义模板；参数为场景、草稿和旧关键词，返回保存结果。 */
  save(scope: TemplateScope, draft: Pick<InputTemplate, "name" | "content">, current?: string) {
    return apiRequest<InputTemplate>(path(scope, current), {
      method: current === undefined ? "POST" : "PUT", body: JSON.stringify(draft)
    });
  },
  /** 删除自定义模板；参数为场景及关键词，返回删除结果。 */
  remove(scope: TemplateScope, name: string) {
    return apiRequest<{ removed: boolean }>(path(scope, name), { method: "DELETE" });
  }
};
