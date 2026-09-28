import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";

/** 独立文档草稿；作用域为所属设置分区的路径。 */
export type SettingsDraft = {
  scope: string;
  dirty: boolean;
  saving: boolean;
  discard: () => void;
  save?: () => Promise<unknown>;
  /** 同分区内哪些查询参数仍属于当前文档。 */
  contains?: (search: string) => boolean;
};

/**
 * 判断导航是否会离开未保存文档。
 * @param draft 独立文档
 * @param location 目标地址
 * @returns 是否需要保护
 */
export function leavesSettingsDraft(draft: SettingsDraft, location: { pathname: string; search: string }): boolean {
  return draft.dirty && (draft.scope !== location.pathname || draft.contains?.(location.search) === false);
}
type Registry = { drafts: SettingsDraft[]; register: (draft: SettingsDraft) => void; unregister: (scope: string) => void };
const DraftContext = createContext<Registry | null>(null);

/**
 * 【设置】【草稿登记】保存独立文档状态，供单一导航守卫与快捷键使用。
 * @param props 设置页面内容
 * @returns 草稿登记上下文
 */
export function SettingsDraftProvider({ children }: { children: ReactNode }) {
  const [drafts, setDrafts] = useState<SettingsDraft[]>([]);
  const register = useCallback((draft: SettingsDraft) => setDrafts((current) => [...current.filter((item) => item.scope !== draft.scope), draft]), []);
  const unregister = useCallback((scope: string) => setDrafts((current) => current.filter((item) => item.scope !== scope)), []);
  const value = useMemo(() => ({ drafts, register, unregister }), [drafts, register, unregister]);
  return <DraftContext.Provider value={value}>{children}</DraftContext.Provider>;
}

/** 读取已登记文档；无上下文的单元组件返回空列表。 */
export function useSettingsDrafts(): SettingsDraft[] {
  return useContext(DraftContext)?.drafts ?? [];
}

/**
 * 【设置】【文档保护】登记当前文档，更新操作引用时不重复触发页面渲染。
 * @param draft 作用域、编辑状态与保存/放弃操作
 * @returns 无返回值
 */
export function useSettingsDraft(draft: SettingsDraft): void {
  const registry = useContext(DraftContext);
  const register = registry?.register;
  const unregister = registry?.unregister;
  const latest = useRef(draft);
  latest.current = draft;
  const { scope, dirty, saving } = draft;
  useEffect(() => {
    register?.({ scope, dirty, saving, discard: () => latest.current.discard(), save: () => latest.current.save?.() ?? Promise.resolve(), contains: (search) => latest.current.contains?.(search) ?? true });
  }, [register, scope, dirty, saving]);
  useEffect(() => () => unregister?.(scope), [unregister, scope]);
}
