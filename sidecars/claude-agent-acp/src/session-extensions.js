import { SAI_EXTENSION_NAMESPACE } from "./extensions.js";

const SESSION_METHODS = [
  "newSession",
  "loadSession",
  "resumeSession",
  "unstable_forkSession",
];

/**
 * 将 Sai Skills 元数据转换为 Claude Agent SDK 支持的系统提示配置。
 *
 * @param {Record<string, unknown>} params ACP 会话参数
 * @returns {Record<string, unknown>} 转换后的会话参数
 */
export function withSaiSkillsSystemPrompt(params) {
  const meta = params?._meta;
  if (!meta || typeof meta !== "object" || Array.isArray(meta)) {
    return params;
  }
  if (meta.systemPrompt !== undefined) {
    return params;
  }
  const sai = meta[SAI_EXTENSION_NAMESPACE];
  if (!sai || typeof sai !== "object" || Array.isArray(sai)) {
    return params;
  }
  const skills = sai.skills;
  if (typeof skills !== "string" || skills.trim().length === 0) {
    return params;
  }
  return {
    ...params,
    _meta: {
      ...meta,
      systemPrompt: {
        type: "preset",
        preset: "claude_code",
        append: skills,
      },
    },
  };
}

/**
 * 包装 Claude ACP 会话入口，使 Sai Skills 进入 Claude Agent SDK。
 *
 * @param {Record<string, Function>} agent Claude ACP agent 实例
 * @returns {Record<string, Function>} 已安装扩展的原实例
 */
export function applySaiSessionExtensions(agent) {
  for (const methodName of SESSION_METHODS) {
    const original = agent[methodName];
    if (typeof original !== "function") {
      continue;
    }
    agent[methodName] = function saiSessionMethod(params) {
      return original.call(this, withSaiSkillsSystemPrompt(params));
    };
  }
  return agent;
}
