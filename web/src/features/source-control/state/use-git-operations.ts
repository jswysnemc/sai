import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useCallback, useRef, useState } from "react";
import { api } from "../../../api/client";
import { ApiError, LocalizedError, toDisplayError } from "../../../api/api-error";
import type { GitOperationResponse, GitRepositoryState } from "../../../api/contracts";
import type { GitOperationAction, GitOperationOptions } from "../../../api/git-contracts";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { useI18n } from "../../i18n/use-i18n";
import type { GitOperationUiOptions, GitOutputEntry } from "../types";
import { allRefreshKeys, refreshKeysFor } from "./git-refresh-keys";

const MAX_OUTPUT_ENTRIES = 50;

type UseGitOperationsOptions = {
  /** 当前选中的仓库根路径，作为操作的默认目标 */
  selectedRoot: string | null;
  /** 操作返回新状态时同步到仓库状态缓存 */
  onStateUpdated: (state: GitRepositoryState) => void;
};

/**
 * 聚合 Git 操作的执行、输出记录与缓存失效。
 *
 * 缓存失效按操作类型收敛到相关查询：早先每次操作都失效十余个查询键，
 * 单击暂存也会连带重取提交历史和远端资源，大仓库上这是主要的卡顿来源。
 *
 * @param options 选中仓库与状态同步回调
 * @returns 操作执行器、忙碌标志、输出记录与提示信息
 */
export function useGitOperations({ selectedRoot, onStateUpdated }: UseGitOperationsOptions) {
  const confirm = useConfirm();
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [error, setError] = useState<Error | null>(null);
  const [notice, setNotice] = useState("");
  const [outputEntries, setOutputEntries] = useState<GitOutputEntry[]>([]);
  const pendingActionRef = useRef<string>("operation");

  /**
   * 追加一条操作输出，超出上限时丢弃最早的记录。
   *
   * @param ok 操作是否成功
   * @param message 操作摘要
   * @param stdout Git 标准输出
   * @param stderr Git 标准错误
   * @returns 无
   */
  const appendOutput = useCallback((ok: boolean, message: string, stdout: string, stderr: string) => {
    setOutputEntries((current) => [
      ...current.slice(-(MAX_OUTPUT_ENTRIES - 1)),
      {
        id: Date.now() * 100 + current.length,
        action: pendingActionRef.current,
        ok,
        message,
        stdout,
        stderr,
        createdAt: Date.now(),
      },
    ]);
  }, []);

  /**
   * 按查询键前缀批量失效缓存。
   *
   * @param keys 需要失效的查询键前缀
   * @returns 全部失效完成的 Promise
   */
  const invalidate = useCallback(
    async (keys: string[]) => {
      await Promise.all(keys.map((key) => queryClient.invalidateQueries({ queryKey: [key] })));
    },
    [queryClient]
  );

  const operation = useMutation({
    mutationFn: (input: { action: GitOperationAction; options: GitOperationOptions }) =>
      api.workspace.gitOp(input.action, input.options),
    onSuccess: async (result, input) => {
      appendOutput(result.ok, result.message, result.stdout, result.stderr);
      // 操作响应已带回最新状态，先写入缓存再失效，避免界面闪回旧值
      queryClient.setQueryData(["git-status", input.options.repo_root ?? selectedRoot], result.state);
      onStateUpdated(result.state);
      if (!result.ok) {
        setError(
          result.message || result.stderr
            ? new ApiError(result.message || result.stderr)
            : new LocalizedError("Git operation failed", "Git 操作失败")
        );
        setNotice("");
        return;
      }
      setError(null);
      setNotice(result.message);
      // 状态已由响应写入，只失效该操作真正影响到的其余查询
      await invalidate(refreshKeysFor(input.action).filter((key) => key !== "git-status"));
    },
    onError: (reason) => {
      const displayError = toDisplayError(reason, "Git operation failed", "Git 操作失败");
      appendOutput(false, displayError.message, "", displayError.message);
      setError(displayError);
      setNotice("");
    },
  });

  const clone = useMutation({
    mutationFn: (input: { remoteUrl: string; directory?: string; parent: string }) =>
      api.workspace.gitClone(input.remoteUrl, input.parent, input.directory),
  });

  /**
   * 执行一次 Git 操作，必要时先弹出确认。
   *
   * @param action 操作标识
   * @param options 操作参数，可携带确认文案
   * @returns 操作响应；被取消或失败时为 undefined
   */
  const runOperation = useCallback(
    async (
      action: GitOperationAction,
      options: GitOperationUiOptions = {}
    ): Promise<GitOperationResponse | undefined> => {
      if (options.confirmTitle) {
        const confirmed = await confirm({
          title: options.confirmTitle,
          description:
            options.confirmDescription ?? t("This action may not be reversible.", "此操作可能无法撤销。"),
          confirmLabel: t("Continue", "继续"),
          danger: true,
        });
        if (!confirmed) return undefined;
      }
      setError(null);
      setNotice("");
      const { confirmTitle: _title, confirmDescription: _description, ...operationOptions } = options;
      pendingActionRef.current = action;
      try {
        return await operation.mutateAsync({
          action,
          options: { ...operationOptions, repo_root: operationOptions.repo_root ?? selectedRoot ?? undefined },
        });
      } catch {
        return undefined;
      }
    },
    [confirm, operation, selectedRoot, t]
  );

  return {
    runOperation,
    clone,
    appendOutput,
    setPendingAction: (action: string) => {
      pendingActionRef.current = action;
    },
    busy: operation.isPending || clone.isPending,
    error,
    setError,
    notice,
    setNotice,
    outputEntries,
    refreshAll: useCallback(() => invalidate(allRefreshKeys()), [invalidate]),
  };
}
