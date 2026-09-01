import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, ChevronDown, ChevronRight, Link2, Pencil, Trash2, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { api } from "../../../api/client";
import type { MemoryQuery, MemoryScope, MemorySummary, MemoryType, MemoryWriteResult } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import { missingRationaleMarkers } from "./memory-filter";

/** 类型徽章的双语文案。 */
const TYPE_LABELS: Record<MemoryType, { en: string; zh: string }> = {
  user: { en: "User", zh: "用户" },
  feedback: { en: "Feedback", zh: "要求" },
  project: { en: "Project", zh: "项目" },
  reference: { en: "Reference", zh: "资源" }
};

/** 作用域徽章的双语文案。 */
const SCOPE_LABELS: Record<MemoryScope, { en: string; zh: string }> = {
  global: { en: "Global", zh: "全局" },
  project: { en: "Project", zh: "项目" }
};

type MemoryEntryCardProps = {
  entry: MemorySummary;
  workspace?: string;
  onRemove: (name: string) => void;
  /** 点击 [[链接]] 时跳转到目标条目 */
  onNavigate: (name: string) => void;
  /** 外部要求展开某条时置为真（链接跳转用） */
  expandSignal?: number;
};

/**
 * 单条记忆的展开卡片：查看正文、就地编辑、删除与链接跳转。
 *
 * 列表接口只带摘要，正文按需拉取：多数时候用户只想确认某条存不存在，
 * 一次列出全部正文会把设置页拖慢。编辑保存走同一个写入接口——
 * 写入同名标识就是就地更新，这比删了重建干净。
 *
 * @param props 条目摘要、工作区标识与操作回调
 * @returns 记忆卡片
 */
export function MemoryEntryCard({
  entry,
  workspace,
  onRemove,
  onNavigate,
  expandSignal = 0
}: MemoryEntryCardProps) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const rootRef = useRef<HTMLElement>(null);
  const [expanded, setExpanded] = useState(false);
  const [editing, setEditing] = useState(false);
  const [saveNote, setSaveNote] = useState<string | null>(null);
  const query: MemoryQuery = { workspace };

  const detail = useQuery({
    queryKey: ["memory-detail", entry.name, workspace],
    queryFn: () => api.memory.show(entry.name, query),
    enabled: expanded
  });

  useEffect(() => {
    if (expandSignal <= 0) return;
    setExpanded(true);
    // 跳转目标可能在视口外：展开后滚到可见位置，否则用户看不到任何反馈
    rootRef.current?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }, [expandSignal]);

  const save = useMutation({
    mutationFn: (request: Parameters<typeof api.memory.remember>[0]) =>
      api.memory.remember(request),
    onSuccess: async (result: MemoryWriteResult) => {
      await queryClient.invalidateQueries({ queryKey: ["memory-detail", entry.name, workspace] });
      await queryClient.invalidateQueries({ queryKey: ["memory-entries"] });
      await queryClient.invalidateQueries({ queryKey: ["memory-stats"] });
      setEditing(false);
      // 软提示放在编辑区里，不弹窗打断
      setSaveNote(result.note ?? null);
    }
  });

  const missing =
    detail.data?.found && detail.data.type && detail.data.content
      ? missingRationaleMarkers(detail.data.type, detail.data.content)
      : [];

  return (
    <article ref={rootRef} className="memory-item" data-type={entry.type}>
      <header className="memory-item-head">
        <button
          type="button"
          className="memory-item-toggle"
          onClick={() => setExpanded((value) => !value)}
          aria-expanded={expanded}
        >
          {expanded ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
          <code>{entry.name}</code>
        </button>
        <span className="memory-type-badge" data-type={entry.type}>
          {t(TYPE_LABELS[entry.type].en, TYPE_LABELS[entry.type].zh)}
        </span>
        <span className="memory-scope-badge" data-scope={entry.scope}>
          {t(SCOPE_LABELS[entry.scope].en, SCOPE_LABELS[entry.scope].zh)}
        </span>
        {expanded && !editing && (
          <button
            type="button"
            onClick={() => setEditing(true)}
            aria-label={t("Edit memory", "编辑记忆")}
            title={t("Edit in place; saving the same identifier updates it", "就地编辑；保存同名标识即更新")}
          >
            <Pencil size={13} />
          </button>
        )}
        <button type="button" onClick={() => onRemove(entry.name)} aria-label={t("Delete memory", "删除记忆")}>
          <Trash2 size={13} />
        </button>
      </header>
      <p>{entry.description}</p>
      {expanded && (
        <div className="memory-item-body">
          {detail.isFetching && <div className="settings-muted">{t("Loading…", "读取中…")}</div>}
          {detail.data?.found && !editing && (
            <>
              <pre>{detail.data.content}</pre>
              {missing.length > 0 && (
                <div className="memory-rationale-hint">
                  {t(
                    `Missing ${missing.join(" and ")} — without them a later turn cannot judge whether this still applies.`,
                    `缺 ${missing.join(" 与 ")}——缺了理由，下一轮无法判断这条在新情境下还适不适用。`
                  )}
                </div>
              )}
              {(detail.data.links?.length ?? 0) > 0 && (
                <div className="memory-item-links">
                  <Link2 size={13} />
                  {detail.data.links?.map((link) => (
                    <button
                      key={link}
                      type="button"
                      className="memory-link-chip"
                      onClick={() => onNavigate(link)}
                      title={t("Jump to this memory", "跳转到这条记忆")}
                    >
                      [[{link}]]
                    </button>
                  ))}
                </div>
              )}
            </>
          )}
          {detail.data?.found && editing && (
            <MemoryEditForm
              name={entry.name}
              description={detail.data.description ?? entry.description}
              content={detail.data.content ?? ""}
              hook={detail.data.hook ?? ""}
              memoryType={detail.data.type ?? entry.type}
              global={entry.scope === "global"}
              workspace={workspace}
              pending={save.isPending}
              onCancel={() => setEditing(false)}
              onSave={(request) => save.mutate(request)}
            />
          )}
          {saveNote && <div className="memory-rationale-hint">{saveNote}</div>}
          {save.isError && (
            <div className="settings-inline-error">{(save.error as Error).message}</div>
          )}
          {detail.data && !detail.data.found && (
            <div className="settings-muted">{t("Entry file is missing", "条目文件已不存在")}</div>
          )}
        </div>
      )}
    </article>
  );
}

type MemoryEditFormProps = {
  name: string;
  description: string;
  content: string;
  /** 索引里的提示行；保存时原样带回，空值后端会沿用摘要 */
  hook: string;
  memoryType: MemoryType;
  global: boolean;
  workspace?: string;
  pending: boolean;
  onCancel: () => void;
  onSave: (request: Parameters<typeof api.memory.remember>[0]) => void;
};

/**
 * 就地编辑表单：改摘要、索引提示与正文。
 *
 * 标识、类型与作用域不在编辑范围：标识是文件名与链接目标，改名等于
 * 换一条记忆；类型与作用域选错，删了重建比改更不容易出半吊子状态。
 *
 * @param props 原值与操作回调
 * @returns 编辑表单
 */
function MemoryEditForm({
  name,
  description,
  content,
  hook,
  memoryType,
  global,
  workspace,
  pending,
  onCancel,
  onSave
}: MemoryEditFormProps) {
  const { t } = useI18n();
  const [nextDescription, setNextDescription] = useState(description);
  const [nextHook, setNextHook] = useState(hook);
  const [nextContent, setNextContent] = useState(content);
  const missing = missingRationaleMarkers(memoryType, nextContent);

  return (
    <div className="memory-edit">
      <label className="memory-compose-field">
        <span>{t("Summary", "摘要")}</span>
        <input
          value={nextDescription}
          onChange={(event) => setNextDescription(event.target.value)}
        />
      </label>
      <label className="memory-compose-field">
        <span>{t("Index hook", "索引提示")}</span>
        <input
          value={nextHook}
          onChange={(event) => setNextHook(event.target.value)}
          placeholder={t("Optional; defaults to the summary", "可选；留空沿用摘要")}
        />
      </label>
      <textarea
        value={nextContent}
        onChange={(event) => setNextContent(event.target.value)}
        rows={6}
      />
      {missing.length > 0 && (
        <div className="memory-rationale-hint">
          {t(
            `Missing ${missing.join(" and ")} — without them a later turn cannot judge whether this still applies.`,
            `缺 ${missing.join(" 与 ")}——缺了理由，下一轮无法判断这条在新情境下还适不适用。`
          )}
        </div>
      )}
      <div className="memory-edit-actions">
        <button type="button" className="settings-secondary" onClick={onCancel} disabled={pending}>
          <X size={13} /> {t("Cancel", "取消")}
        </button>
        <button
          type="button"
          disabled={pending || nextContent.trim().length === 0 || nextDescription.trim().length === 0}
          onClick={() =>
            onSave({
              name,
              description: nextDescription.trim(),
              content: nextContent,
              memory_type: memoryType,
              global,
              hook: nextHook.trim(),
              workspace
            })
          }
        >
          <Check size={13} /> {pending ? t("Saving", "保存中") : t("Save changes", "保存修改")}
        </button>
      </div>
      {nextDescription.trim().length === 0 && (
        <div className="memory-rationale-hint">
          {t("A summary is required.", "摘要不能为空。")}
        </div>
      )}
    </div>
  );
}
