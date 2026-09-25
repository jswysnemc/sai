import { createContext, useContext } from "react";

/** 单个变更文件的增删行数。 */
export type ChangeFileStat = {
  added: number;
  removed: number;
};

/** 当前审阅补丁里各文件的行数统计。 */
export const ChangeFileStatsContext = createContext<ReadonlyMap<string, ChangeFileStat>>(new Map());

/** 变更列表的路径筛选词。 */
export const ChangeListQueryContext = createContext("");

/** 递增后展开变更列表里被折叠的目录。 */
export const ChangeListExpandContext = createContext(0);

/**
 * 读取某个文件在当前补丁中的增删行数。
 *
 * @param path 仓库相对路径
 * @returns 行数统计；补丁里没有该文件时为空
 */
export function useChangeFileStat(path: string): ChangeFileStat | undefined {
  return useContext(ChangeFileStatsContext).get(path);
}
