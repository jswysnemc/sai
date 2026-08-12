import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useI18n } from "../../i18n/use-i18n";
import { GIT_EVENT_TYPES, gitEventsUrl, parseGitWatchEvent, type GitWatchEvent } from "../events/git-events";

export type GitWatchMode = "changes" | "history" | "repositories";

/**
 * 订阅 Git 文件变化，并按当前视图失效相关查询。
 *
 * @param repoRoot 当前仓库或 worktree 根目录
 * @param enabled 是否建立事件连接
 * @param mode 当前 Source Control 子视图
 * @returns 监听器错误详情
 */
export function useGitRepositoryEvents(
  repoRoot: string | null,
  enabled: boolean,
  mode: GitWatchMode
): string {
  const queryClient = useQueryClient();
  const { t } = useI18n();
  const [error, setError] = useState("");
  const modeRef = useRef(mode);

  useEffect(() => {
    modeRef.current = mode;
    if (enabled && mode === "repositories") {
      void queryClient.invalidateQueries({ queryKey: ["git-repositories"] });
    }
  }, [enabled, mode, queryClient]);

  useEffect(() => {
    if (!enabled) return;
    const source = new EventSource(gitEventsUrl(repoRoot));
    // 前端二次合并：服务端 500ms 合并窗口之外，连续事件（保存风暴、
    // git 命令批量改 refs）也只触发一轮查询失效，避免并发重拉
    let flushTimer: number | null = null;
    let pendingMetadataChanged = false;
    let hasPendingEvent = false;

    /** 根据合并后的变化类型刷新当前活跃的 Git 查询。 */
    const invalidate = async (metadataChanged: boolean) => {
      const currentMode = modeRef.current;
      const requests = [
        queryClient.invalidateQueries({ queryKey: ["git-status", repoRoot] }),
        queryClient.invalidateQueries({ queryKey: ["git-statuses"] }),
        queryClient.invalidateQueries({ queryKey: ["git-review-diff", repoRoot] }),
        queryClient.invalidateQueries({ queryKey: ["git-conflict", repoRoot] })
      ];
      if (metadataChanged || currentMode === "history") {
        requests.push(
          queryClient.invalidateQueries({ queryKey: ["git-branches", repoRoot] }),
          queryClient.invalidateQueries({ queryKey: ["git-log", repoRoot] }),
          queryClient.invalidateQueries({ queryKey: ["git-resources", repoRoot] }),
          queryClient.invalidateQueries({ queryKey: ["git-commit-details", repoRoot] }),
          queryClient.invalidateQueries({ queryKey: ["git-commit-diff", repoRoot] })
        );
      }
      if (metadataChanged || currentMode === "repositories") {
        requests.push(queryClient.invalidateQueries({ queryKey: ["git-repositories"] }));
      }
      await Promise.all(requests);
    };

    /** 合并事件并延迟触发一轮失效。 */
    const scheduleInvalidate = (event: GitWatchEvent) => {
      pendingMetadataChanged = pendingMetadataChanged || event.repository_metadata_changed;
      hasPendingEvent = true;
      if (flushTimer !== null) return;
      flushTimer = window.setTimeout(() => {
        flushTimer = null;
        if (!hasPendingEvent) return;
        const metadataChanged = pendingMetadataChanged;
        pendingMetadataChanged = false;
        hasPendingEvent = false;
        void invalidate(metadataChanged);
      }, 250);
    };

    /** 解析单条 SSE 消息，并展示服务端监听错误。 */
    const handle = (message: MessageEvent<string>) => {
      try {
        const event = parseGitWatchEvent(message.data);
        setError(event.error ?? "");
        if (event.paths.length > 0) scheduleInvalidate(event);
      } catch (reason) {
        const detail = reason instanceof Error ? reason.message : String(reason);
        setError(t(`Invalid Git repository event: ${detail}`, `Git 仓库事件格式无效：${detail}`));
      }
    };

    for (const type of GIT_EVENT_TYPES) source.addEventListener(type, handle as EventListener);
    source.onopen = () => setError("");
    return () => {
      if (flushTimer !== null) window.clearTimeout(flushTimer);
      source.close();
    };
  }, [enabled, queryClient, repoRoot, t]);

  return error;
}
