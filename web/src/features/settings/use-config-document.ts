import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";

type ConfigDocumentOptions<Config, Response> = {
  /** React Query 缓存键 */
  queryKey: readonly unknown[];
  /** 加载配置文档 */
  load: () => Promise<Response>;
  /** 从加载/保存响应中取配置对象 */
  extract: (response: Response) => Config;
  /** 保存配置文档 */
  save: (config: Config) => Promise<Response>;
  /** 保存期间继续编辑时，仅同步服务端生成的对象身份，保留本地字段 */
  rebase?: (draft: Config, submitted: Config, saved: Config) => Config;
  /** 保存成功后的附加副作用（缓存失效等） */
  onSaved?: (response: Response, queryClient: QueryClient) => Promise<void> | void;
};

/**
 * 配置文档控制器：加载、草稿、脏标记与保存的统一实现。
 *
 * 全局 AppConfig 与独立 MCP 文档此前各写一份同构状态机，此处合一；
 * 领域方法（供应商更新、服务列表编辑）由各自的外层 Hook 组合。
 *
 * @param options 文档的加载/保存端点与副作用
 * @returns 文档状态与操作
 */
export function useConfigDocument<Config, Response>({
  queryKey,
  load,
  save,
  extract,
  rebase,
  onSaved
}: ConfigDocumentOptions<Config, Response>) {
  const queryClient = useQueryClient();
  const response = useQuery({ queryKey, queryFn: load });
  const [draft, setDraft] = useState<Config | null>(null);
  const [dirty, setDirty] = useState(false);
  const revision = useRef(0);
  const pending = useRef<Promise<Response> | null>(null);

  useEffect(() => {
    if (!response.data || dirty) return;
    // 1. 仅在无本地草稿时用服务端快照重置
    setDraft(extract(response.data));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [response.data, dirty]);

  const mutation = useMutation({
    mutationFn: (submission: { config: Config; revision: number }) => save(submission.config),
    onSuccess: async (saved, submission) => {
      // 1. 【设置】【保存回填】仅覆盖本次提交的版本，保留请求期间的新编辑
      if (revision.current === submission.revision) {
        setDraft(extract(saved));
        setDirty(false);
      } else if (rebase) {
        setDraft(current => current === null ? null : rebase(current, submission.config, extract(saved)));
      }
      queryClient.setQueryData(queryKey, saved);
      await onSaved?.(saved, queryClient);
    }
  });

  /**
   * 用完整配置替换草稿并标记待保存。
   *
   * @param next 新配置对象
   */
  const update = (next: Config) => {
    revision.current += 1;
    setDraft(next);
    setDirty(true);
    if (!pending.current) mutation.reset();
  };

  /**
   * 仅标记待保存并清除上次保存错误。
   *
   * 供文本草稿暂不合法、无法回写结构化对象的编辑路径使用。
   */
  const markDirty = () => {
    revision.current += 1;
    setDirty(true);
    if (!pending.current) mutation.reset();
  };

  /**
   * 提交指定配置；缺省提交当前草稿。
   *
   * @param config 覆盖提交内容（如 JSON 模式的解析结果）
   * @returns 服务端保存响应
   */
  const saveNow = async (config?: Config): Promise<Response> => {
    if (pending.current) return pending.current;
    const payload = config ?? draft;
    if (payload == null) throw new Error("Configuration is not ready to save");
    const request = mutation.mutateAsync({ config: payload, revision: revision.current });
    pending.current = request;
    try {
      return await request;
    } finally {
      pending.current = null;
    }
  };

  /**
   * 放弃未保存修改，把草稿恢复为服务端快照。
   *
   * @returns 无返回值
   */
  const discard = () => {
    revision.current += 1;
    if (response.data) setDraft(extract(response.data));
    setDirty(false);
    if (!pending.current) mutation.reset();
  };

  return {
    /** 原始加载查询，路径、sentinel 等响应级字段由调用方读取 */
    response,
    draft,
    /** 服务端最近一次加载或保存的配置快照 */
    baseline: response.data ? extract(response.data) : null,
    dirty,
    loading: response.isLoading,
    saving: mutation.isPending,
    saved: mutation.isSuccess && !dirty,
    loadError: (response.error ?? null) as Error | null,
    saveError: (mutation.error ?? null) as Error | null,
    update,
    markDirty,
    saveNow,
    discard,
    /** 重新拉取配置 */
    retry: () => void response.refetch()
  };
}
