/** Sai 已为 Codex ACP 接通的宿主集成能力。 */
export const SAI_CAPABILITIES = Object.freeze({
  context_compaction: true,
  memory: true,
  goal_continuation: true,
  subagents: true,
});

/**
 * 记录一条客户端 JSONL 消息中的 initialize 请求标识。
 *
 * @param {string} line 客户端输入行
 * @param {Set<string>} initializeIds 等待响应的 initialize 标识集合
 * @returns {string} 原始输入行
 */
export function trackInitializeRequest(line, initializeIds) {
  const message = parseObject(line);
  if (message?.method === "initialize" && Object.hasOwn(message, "id")) {
    initializeIds.add(idKey(message.id));
  }
  return line;
}

/**
 * 为匹配的 initialize 响应补充 Sai 能力声明。
 *
 * @param {string} line Codex ACP 输出行
 * @param {Set<string>} initializeIds 等待响应的 initialize 标识集合
 * @returns {string} 修改后的 JSONL 行；无法解析或不匹配时返回原文
 */
export function extendInitializeResponse(line, initializeIds) {
  // 【Codex ACP Sidecar】【能力扩展】1. 只处理与已记录 initialize 请求匹配的成功响应
  const message = parseObject(line);
  if (!message || !Object.hasOwn(message, "id")) {
    return line;
  }
  const key = idKey(message.id);
  if (!initializeIds.delete(key) || !isRecord(message.result)) {
    return line;
  }
  const meta = isRecord(message.result._meta) ? message.result._meta : {};
  const sai = isRecord(meta._sai) ? meta._sai : {};
  const capabilities = isRecord(sai.capabilities) ? sai.capabilities : {};
  const nativeEquivalents = isRecord(sai.native_equivalents) ? sai.native_equivalents : {};
  // 【Codex ACP Sidecar】【能力扩展】2. 保留 agent 既有元数据并补充 Sai 已接通的能力
  message.result._meta = {
    ...meta,
    _sai: {
      ...sai,
      capabilities: {
        ...capabilities,
        ...SAI_CAPABILITIES,
      },
      native_equivalents: {
        ...nativeEquivalents,
        context_compaction: "codex",
        subagents: "codex",
      },
    },
  };
  return JSON.stringify(message);
}

/**
 * 将 JSON-RPC 标识转换为类型稳定的集合键。
 *
 * @param {unknown} id JSON-RPC 请求标识
 * @returns {string} 可区分字符串与数字的稳定键
 */
function idKey(id) {
  return `${typeof id}:${JSON.stringify(id)}`;
}

/**
 * 解析一行 JSON 并确认结果为普通对象。
 *
 * @param {string} line JSONL 行
 * @returns {Record<string, unknown> | null} 普通对象；解析失败时返回空值
 */
function parseObject(line) {
  try {
    const value = JSON.parse(line);
    return isRecord(value) ? value : null;
  } catch {
    return null;
  }
}

/**
 * 判断值是否为普通记录对象。
 *
 * @param {unknown} value 待判断值
 * @returns {value is Record<string, unknown>} 普通对象时返回 true
 */
function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
