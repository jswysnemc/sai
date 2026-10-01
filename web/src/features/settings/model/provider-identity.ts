import type { AppConfig } from "../../../api/contracts";

/**
 * 【供应商设置】【保存回填】保存期间继续改名时更新密钥来源，保留最新草稿字段。
 * @param draft 仍在编辑的草稿
 * @param submitted 本次保存提交的快照
 * @param saved 服务端保存后的快照
 * @returns 同步密钥来源后的草稿
 */
export function rebaseProviderSources(draft: AppConfig, submitted: AppConfig, saved: AppConfig): AppConfig {
  const sources = new Map(submitted.providers.flatMap(provider => {
    const persisted = saved.providers.find(item => item.id === provider.id);
    return persisted ? [[provider.original_id ?? provider.id, persisted.original_id ?? persisted.id] as const] : [];
  }));
  return {
    ...draft,
    providers: draft.providers.map(provider => {
      const source = sources.get(provider.original_id ?? provider.id);
      return source ? { ...provider, original_id: source } : provider;
    })
  };
}

/**
 * 【供应商设置】【关联改名】同步各功能明确声明的供应商引用，保留用户自定义内容。
 * @param config 完整草稿
 * @param previousId 改名前的供应商标识
 * @param nextId 改名后的供应商标识
 * @returns 更新引用后的草稿
 */
export function renameProviderReferences(config: AppConfig, previousId: string, nextId: string): AppConfig {
  if (previousId === nextId) return config;
  /**
   * 更新指定对象的直接引用字段。
   * @param value 配置对象
   * @param fields 该对象使用的供应商引用字段
   * @returns 原对象或更新后的副本
   */
  const rename = <T extends object>(value: T | undefined, fields: string[]): T | undefined => {
    if (!value) return value;
    const next = { ...value } as Record<string, unknown>;
    for (const field of fields) if (next[field] === previousId) next[field] = nextId;
    return next as T;
  };
  return {
    ...config,
    active_provider: config.active_provider === previousId ? nextId : config.active_provider,
    session: rename(config.session, ["new_session_provider_id", "auto_title_provider_id"]),
    context: rename(config.context, ["compaction_provider_id"]),
    memory: rename(config.memory, ["extraction_provider_id"]),
    permission: rename(config.permission, ["auto_audit_provider_id"]),
    git: rename(config.git, ["auto_commit_message_provider_id"]),
    agents: config.agents?.map(agent => rename(agent, ["provider_id"])!),
    subagent: config.subagent ? {
      ...rename(config.subagent, ["provider_id"]),
      profiles: config.subagent.profiles?.map(profile => rename(profile, ["provider_id"])!),
      model_overrides: config.subagent.model_overrides && Object.fromEntries(
        Object.entries(config.subagent.model_overrides).map(([key, value]) => [key, rename(value, ["provider_id"])!])
      )
    } : config.subagent,
    plugins: config.plugins ? {
      ...config.plugins,
      ...(config.plugins.vision ? { vision: rename(config.plugins.vision, ["vision_provider_id"])! } : {})
    } : config.plugins
  };
}
