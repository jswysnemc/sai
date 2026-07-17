import { Check, Copy, FileText, Save, Trash2 } from "lucide-react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import type { AppConfig, PromptKind } from "../../api/contracts";
import { EditorHeader } from "./editor-layout";
import { ObjectListPanel } from "./object-list-panel";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { useI18n } from "../i18n/use-i18n";

type PromptSettingsSectionProps = {
  config: AppConfig;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染 AI 人设和用户身份文件管理界面。
 *
 * @param props 应用配置和更新回调
 * @returns 提示词管理区域
 */
export function PromptSettingsSection({ config, onConfigChange }: PromptSettingsSectionProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const [kind, setKind] = useState<PromptKind>("personas");
  const prompts = useQuery({ queryKey: ["prompts", kind], queryFn: () => api.prompts.list(kind) });
  const [selected, setSelected] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [content, setContent] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const promptConfig = config.prompt ?? {};
  const activeName = kind === "personas" ? promptConfig.active_persona ?? "" : promptConfig.active_identity ?? "";

  useEffect(() => {
    setSelected(null);
    setName("");
    setContent("");
    setError(null);
  }, [kind]);

  /** 读取选中的提示词文件。 */
  const selectPrompt = async (nextName: string) => {
    setError(null);
    try {
      const document = await api.prompts.read(kind, nextName);
      setSelected(nextName);
      setName(document.name);
      setContent(document.content);
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to load prompt", "读取提示词失败"));
    }
  };

  /** 创建空白提示词草稿。 */
  const createDraft = () => {
    setSelected(null);
    setName(kind === "personas" ? t("New persona", "新建人设") : t("New identity", "新建身份"));
    setContent("");
    setError(null);
  };

  /** 复制当前提示词为新草稿。 */
  const copyDraft = () => {
    if (!name) return;
    setSelected(null);
    setName(`${name}-copy`);
  };

  /** 保存当前提示词草稿或修改。 */
  const savePrompt = async () => {
    if (!name.trim()) return;
    setSaving(true);
    setError(null);
    try {
      const document = selected
        ? await api.prompts.update(kind, selected, name, content)
        : await api.prompts.create(kind, name, content);
      if (selected && (activeName === selected || activeName === `${selected}.md`)) activatePrompt(document.name);
      setSelected(document.name);
      setName(document.name);
      setContent(document.content);
      await queryClient.invalidateQueries({ queryKey: ["prompts", kind] });
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to save prompt", "保存提示词失败"));
    } finally {
      setSaving(false);
    }
  };

  /** 删除当前选中的提示词文件。 */
  const deletePrompt = async () => {
    if (!selected) return;
    const confirmed = await confirm({
      title: t("Delete prompt", "删除提示词"),
      description: t(`Delete “${selected}” and its associated file?`, `将删除“${selected}”及其关联文件。`),
      confirmLabel: t("Delete", "删除"),
      danger: true
    });
    if (!confirmed) return;
    setError(null);
    try {
      await api.prompts.remove(kind, selected);
      if (activeName === selected || activeName === `${selected}.md`) activatePrompt("");
      setSelected(null);
      setName("");
      setContent("");
      await queryClient.invalidateQueries({ queryKey: ["prompts", kind] });
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to delete prompt", "删除提示词失败"));
    }
  };

  /** 将指定提示词设为当前人设或身份。 */
  const activatePrompt = (value: string) => {
    const key = kind === "personas" ? "active_persona" : "active_identity";
    onConfigChange({ ...config, prompt: { ...promptConfig, [key]: value ? `${value}.md` : "" } });
  };

  const items = prompts.data?.items ?? [];
  return (
    <div className="settings-objects-layout">
      <ObjectListPanel
        title={kind === "personas" ? t("AI personas", "AI 人设") : t("User identities", "用户身份")}
        items={items.map((item) => ({
          id: item.name,
          name: item.name,
          meta: "Markdown",
          icon: <FileText size={14} />,
          marked: activeName === item.name || activeName === `${item.name}.md`
        }))}
        selectedId={selected ?? ""}
        searchPlaceholder={t("Search prompts", "搜索提示词")}
        addLabel={kind === "personas" ? t("Add persona", "新增人设") : t("Add identity", "新增身份")}
        onSelect={(id) => void selectPrompt(id)}
        onAdd={createDraft}
        headerSlot={
          <div className="prompt-kind-tabs">
            <button type="button" className={kind === "personas" ? "active" : ""} onClick={() => setKind("personas")}>{t("AI personas", "AI 人设")}</button>
            <button type="button" className={kind === "identities" ? "active" : ""} onClick={() => setKind("identities")}>{t("User identities", "用户身份")}</button>
          </div>
        }
        topSlot={
          <button type="button" className="prompt-default-row" onClick={() => activatePrompt("")}>
            <span><strong>{kind === "personas" ? t("Built-in Sai", "内置 Sai") : t("Do not use a user identity", "不使用用户身份")}</strong><small>{t("Default configuration", "默认配置")}</small></span>
            {!activeName && <Check size={14} />}
          </button>
        }
      />
      <section className="settings-editor prompt-editor">
        <EditorHeader
          kicker={t("Custom prompts", "自定义提示词")}
          title={selected ? name : name || t("Select or add a prompt", "选择或新增提示词")}
          description={t("Content is stored as Markdown files in the same directory used by the TUI.", "内容以 Markdown 文件保存，并与 TUI 使用相同目录。")}
          actions={<>
            <button type="button" className="settings-secondary" onClick={copyDraft} disabled={!name}><Copy size={14} />{t("Copy", "复制")}</button>
            <button type="button" className="settings-secondary" onClick={() => activatePrompt(name)} disabled={!name || activeName === name || activeName === `${name}.md`}><Check size={14} />{t("Set as current", "设为当前")}</button>
            <button type="button" className="settings-danger" onClick={() => void deletePrompt()} disabled={!selected}><Trash2 size={14} />{t("Delete", "删除")}</button>
          </>}
        />
        <label className="settings-field"><span>{t("Name", "名称")}</span><input value={name} onChange={(event) => setName(event.target.value)} placeholder={t("Prompt name", "提示词名称")} /></label>
        <label className="settings-field prompt-content-field"><span>{t("Prompt content", "提示词内容")}</span><textarea value={content} onChange={(event) => setContent(event.target.value)} placeholder={t("Enter a system prompt or user identity description", "输入系统提示词或用户身份说明")} spellCheck={false} /></label>
        <div className="prompt-editor-footer">
          {error && <span className="settings-inline-error">{error.message}</span>}
          <button type="button" className="settings-save" onClick={() => void savePrompt()} disabled={!name.trim() || saving}><Save size={14} />{saving ? t("Saving", "正在保存") : t("Save prompt", "保存提示词")}</button>
        </div>
      </section>
    </div>
  );
}
