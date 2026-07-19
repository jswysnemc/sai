import { useCallback, useState, type SetStateAction } from "react";
import type { ChangeSectionKind } from "../changes/change-section";
import type { SourceControlDiffView } from "../diff/diff-mode";

type RepositoryScmState = {
  message: string;
  diffMode: SourceControlDiffView;
  selectedPath: string | null;
  selectedSection: ChangeSectionKind;
  selectedCommit: string | null;
  selectedCommitPath: string | null;
  historyLimit: number;
};

const INITIAL_STATE: RepositoryScmState = {
  message: "",
  diffMode: "changes",
  selectedPath: null,
  selectedSection: "changes",
  selectedCommit: null,
  selectedCommitPath: null,
  historyLimit: 40
};

/**
 * 按仓库根目录保存 Source Control 的提交说明、选择和分页状态。
 *
 * @param repositoryRoot 当前仓库根目录
 * @returns 当前仓库状态与字段更新方法
 */
export function useScmStateStore(repositoryRoot: string | null) {
  const key = repositoryRoot ?? "__active_workspace__";
  const [states, setStates] = useState<Record<string, RepositoryScmState>>({});
  const state = states[key] ?? INITIAL_STATE;

  /**
   * 更新当前仓库单个状态字段，并支持 React 函数式更新。
   *
   * @param field 待更新字段
   * @param action 新值或基于旧值的更新函数
   * @returns 无返回值
   */
  const setField = useCallback(<Key extends keyof RepositoryScmState>(
    field: Key,
    action: SetStateAction<RepositoryScmState[Key]>
  ) => {
    setStates((current) => {
      const previous = current[key] ?? INITIAL_STATE;
      const value = typeof action === "function"
        ? (action as (value: RepositoryScmState[Key]) => RepositoryScmState[Key])(previous[field])
        : action;
      return { ...current, [key]: { ...previous, [field]: value } };
    });
  }, [key]);

  return {
    ...state,
    setMessage: (value: SetStateAction<string>) => setField("message", value),
    setDiffMode: (value: SetStateAction<SourceControlDiffView>) => setField("diffMode", value),
    setSelectedPath: (value: SetStateAction<string | null>) => setField("selectedPath", value),
    setSelectedSection: (value: SetStateAction<ChangeSectionKind>) => setField("selectedSection", value),
    setSelectedCommit: (value: SetStateAction<string | null>) => setField("selectedCommit", value),
    setSelectedCommitPath: (value: SetStateAction<string | null>) => setField("selectedCommitPath", value),
    setHistoryLimit: (value: SetStateAction<number>) => setField("historyLimit", value)
  };
}
