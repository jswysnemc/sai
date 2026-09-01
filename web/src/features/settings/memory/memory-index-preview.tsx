import { useQuery } from "@tanstack/react-query";
import { Eye } from "lucide-react";
import { useState } from "react";
import { api } from "../../../api/client";
import type { MemoryQuery } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";

type MemoryIndexPreviewProps = {
  workspace?: string;
};

/**
 * 每轮实际注入给模型的记忆索引预览。
 *
 * 界面上看到的必须和模型看到的一致，否则「明明记过却没生效」无从排查：
 * 索引按工作区渲染，这里跟着同一个工作区取数。
 *
 * @param props 工作区标识
 * @returns 索引预览面板
 */
export function MemoryIndexPreview({ workspace }: MemoryIndexPreviewProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const query: MemoryQuery = { workspace };
  const index = useQuery({
    queryKey: ["memory-index", workspace],
    queryFn: () => api.memory.index(query),
    enabled: open
  });

  return (
    <div className={open ? "memory-tool-panel" : "memory-tool-entry"}>
      <button
        type="button"
        className="memory-tool-toggle"
        onClick={() => setOpen((value) => !value)}
        aria-expanded={open}
      >
        <Eye size={13} />
        {t("Injected index preview", "注入索引预览")}
        {!open && (
          <small>
            {t(
              "Exactly what the model receives each turn",
              "模型每轮收到的索引，与这里显示的一致"
            )}
          </small>
        )}
      </button>
      {open && (
        <div className="memory-index-body">
          {index.isFetching && <div className="settings-muted">{t("Loading…", "读取中…")}</div>}
          {!index.isFetching && index.data && !index.data.injected && (
            <div className="settings-muted">
              {t("No memories yet; nothing is injected.", "还没有记忆，本轮不注入索引。")}
            </div>
          )}
          {!index.isFetching && index.data?.injected && <pre>{index.data.text}</pre>}
          {index.isError && (
            <div className="settings-inline-error">{(index.error as Error).message}</div>
          )}
        </div>
      )}
    </div>
  );
}
