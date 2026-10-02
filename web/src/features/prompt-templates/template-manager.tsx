import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { BookOpen } from "../../shared/ui/icons";
import { Button } from "../../shared/ui/button/button";
import { Modal } from "../../shared/ui/dialog/modal";
import { TextInput } from "../../shared/ui/form/text-input";
import { TextArea } from "../../shared/ui/form/text-area";
import { useI18n } from "../i18n/use-i18n";
import { templateApi, templateKey, type InputTemplate, type TemplateScope } from "./template-client";

/** 渲染当前场景模板管理入口；参数为场景与禁用状态，返回按钮与编辑弹窗。 */
export function TemplateManager({ scope, disabled }: { scope: TemplateScope; disabled: boolean }) {
  const { t } = useI18n();
  const cache = useQueryClient();
  const [open, setOpen] = useState(false);
  const [current, setCurrent] = useState<InputTemplate | null>(null);
  const [name, setName] = useState("");
  const [content, setContent] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [deleting, setDeleting] = useState(false);
  const templates = useQuery({ queryKey: templateKey(scope), queryFn: () => templateApi.list(scope), enabled: open });

  /** 载入模板或清空表单；参数为模板，返回空值。 */
  function select(item: InputTemplate | null) {
    setCurrent(item); setName(item?.name ?? ""); setContent(item?.content ?? "");
    setError(""); setDeleting(false);
  }

  /** 保存或删除当前模板；参数为动作，返回完成状态。 */
  async function mutate(remove = false) {
    setBusy(true); setError("");
    try {
      if (remove && current) await templateApi.remove(scope, current.name);
      else await templateApi.save(scope, { name, content }, current?.builtin ? undefined : current?.name);
      await cache.invalidateQueries({ queryKey: templateKey(scope) });
      select(null);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
    } finally { setBusy(false); }
  }

  const valid = /^[A-Za-z0-9_-]{1,64}$/.test(name) && Boolean(content.trim()) && new TextEncoder().encode(content).length <= 65536;
  return <>
    <Button variant="ghost" size="small" disabled={disabled} onClick={() => setOpen(true)} title={t("Prompt templates · type / to insert", "提示词模板 · 输入 / 插入")}>
      <BookOpen size={14} />{t("Templates", "模板")}
    </Button>
    {open && <Modal open title={scope === "image" ? t("Image prompt templates", "绘图提示词模板") : t("Chat prompt templates", "普通聊天提示词模板")}
      description={t("Type /keyword in the composer to insert a template without sending. Templates are saved on this Sai server.", "在输入框键入 /关键词即可选择并填入模板，不会自动发送。模板保存在当前 Sai 服务端。")}
      size="large" onClose={() => { if (!busy) setOpen(false); }}
      footer={<><Button disabled={busy} onClick={() => setOpen(false)}>{t("Close", "关闭")}</Button>
        <Button variant="primary" disabled={busy || !valid || (current?.builtin === true && name === current.name)} onClick={() => void mutate()}>{busy ? t("Saving…", "正在保存…") : current?.builtin ? t("Save as copy", "另存为副本") : t("Save", "保存")}</Button></>}>
      <div className="grid min-w-0 grid-cols-1 gap-4 text-sm md:grid-cols-2">
        <div className="flex min-w-0 flex-col gap-2">
          <Button disabled={busy} onClick={() => select(null)}>{t("New template", "新建模板")}</Button>
          {templates.isLoading && <p>{t("Loading…", "正在加载…")}</p>}
          {templates.isError && <p role="alert">{String(templates.error.message)}</p>}
          <div className="flex max-h-64 flex-col gap-1 overflow-y-auto">
            {templates.data?.map((item) => <Button key={item.name} variant={current?.name === item.name ? "secondary" : "ghost"} disabled={busy} onClick={() => select(item)}>
              <span className="min-w-0 truncate">/{item.name}</span><span className="ml-auto text-xs">{item.builtin ? t("Built-in", "预置") : t("Custom", "自定义")}</span>
            </Button>)}
          </div>
        </div>
        <div className="flex min-w-0 flex-col gap-3">
          <label className="flex flex-col gap-1">{t("Keyword", "触发关键词")}
            <TextInput value={name} maxLength={64} disabled={busy} placeholder="my-template" onChange={(event) => setName(event.target.value)} />
            <span className="text-xs">{t("1–64 letters, numbers, hyphens or underscores. No leading slash.", "使用 1–64 个英文字母、数字、连字符或下划线，不包含开头斜杠。")}</span>
          </label>
          <label className="flex flex-col gap-1">{t("Prompt", "提示词正文")}
            <TextArea rows={9} value={content} disabled={busy} onChange={(event) => setContent(event.target.value)} />
          </label>
          {current?.builtin && <p className="text-xs">{t("Change the keyword to save a custom copy.", "修改关键词即可保存为自定义副本。")}</p>}
          {current && !current.builtin && <Button variant="ghost-danger" disabled={busy} onClick={() => deleting ? void mutate(true) : setDeleting(true)}>{deleting ? t("Confirm deletion", "确认删除") : t("Delete template", "删除模板")}</Button>}
          {error && <p role="alert" className="break-words">{error}</p>}
        </div>
      </div>
    </Modal>}
  </>;
}
