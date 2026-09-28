import { Terminal, Webhook } from "../../shared/ui/icons";
import type { AppConfig, HookItem } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { useI18n } from "../i18n/use-i18n";
import { ChoicePills, DetailHeader, EmptyGuide, FieldGrid, InlineSwitch, MasterDetail, ObjectList, SettingsField, SettingsPanel, SkNumberInput, SkSelect, SkTextInput, SwitchField } from "./kit";
import { useSettingsItem } from "./shell/use-settings-item";
import { HookActionFields } from "./hooks/hook-action-fields";
import { createHook, HOOK_EVENTS, HOOK_TEMPLATES, hookItemKey } from "./hooks/hook-templates";

type Props = { config: AppConfig; onConfigChange: (config: AppConfig) => void };

/**
 * 【Hooks】【配置】组合生命周期钩子列表、模板与动作编辑。
 * @param props 应用配置与草稿更新回调
 * @returns 钩子设置页
 */
export function HooksSettingsSection({ config, onConfigChange }: Props) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const hooks = config.hooks ?? { enabled: true, items: [] };
  const items = hooks.items ?? [];
  const [selectedId, select] = useSettingsItem(items.map(hookItemKey));
  const index = items.findIndex((item, i) => hookItemKey(item, i) === selectedId);
  const hook = items[index];
  /** 合并钩子配置；参数为顶层补丁，返回无值。 */
  const setHooks = (patch: Partial<NonNullable<AppConfig["hooks"]>>) => onConfigChange({ ...config, hooks: { ...hooks, ...patch } });
  /** 更新所选钩子并同步地址；参数为字段补丁，返回无值。 */
  const update = (patch: Partial<HookItem>) => {
    setHooks({ items: items.map((item, i) => i === index ? { ...item, ...patch } : item) });
    if (patch.name !== undefined) select(hookItemKey({ ...hook, ...patch }, index));
  };
  /** 从可选模板新增钩子；参数为模板补丁，返回无值。 */
  const add = (patch: Partial<HookItem> = {}) => {
    const created = createHook(items, patch);
    setHooks({ items: [...items, created] });
    select(hookItemKey(created, items.length));
  };
  /** 确认后删除所选钩子；返回操作完成的 Promise。 */
  const remove = async () => {
    if (!hook || !await confirm({ title: t("Delete hook", "删除 Hook"), description: t(`Delete “${hook.name}”?`, `删除“${hook.name}”？`), confirmLabel: t("Delete", "删除"), danger: true })) return;
    const next = items.filter((_, i) => i !== index);
    setHooks({ items: next });
    select(next.length ? hookItemKey(next[0], 0) : "");
  };
  const master = <SwitchField label={t("Enable hooks", "启用 Hooks")} anchor="hooks.enabled" checked={hooks.enabled !== false} onChange={(enabled) => setHooks({ enabled })} hint={t("Hook failures are logged and do not block the main run.", "钩子失败只记录日志，不阻断主流程。")} />;
  if (!hook) return <div className="grid gap-4">
    {master}
    <EmptyGuide title={t("No hooks configured", "尚未配置 Hook")} description={t("Start from a diagnostic template or add your own command or HTTP action.", "可从诊断模板开始，或添加自定义命令与 HTTP 动作。")} action={<Button variant="secondary" onClick={() => add()}>{t("Add hook", "添加 Hook")}</Button>} templates={HOOK_TEMPLATES.map((template) => ({ id: template.id, label: t(template.en, template.zh), description: t(template.descriptionEn, template.descriptionZh), onSelect: () => add(template.patch) }))} />
  </div>;
  return <MasterDetail list={<ObjectList title="Hooks" items={items.map((item, i) => ({ id: hookItemKey(item, i), name: item.name, meta: item.event, icon: item.kind === "http" ? <Webhook size={14} /> : <Terminal size={14} />, marked: item.enabled !== false }))} selectedId={selectedId} onSelect={select} onAdd={() => add()} searchPlaceholder={t("Search hooks", "搜索 Hook")} headerSlot={master} />}>
    <DetailHeader title={hook.name} actions={<InlineSwitch label={t("Enabled", "已启用")} checked={hook.enabled !== false} onChange={(enabled) => update({ enabled })} />} menuItems={[{ id: "delete", label: t("Delete hook", "删除 Hook"), danger: true, onSelect: () => void remove() }]} />
    <SettingsPanel title={t("Trigger", "触发条件")}>
      <FieldGrid>
        <SettingsField label={t("Name", "名称")} anchor="hooks.name"><SkTextInput value={hook.name} onChange={(name) => update({ name })} /></SettingsField>
        <SettingsField label={t("Event", "事件")} anchor="hooks.event"><SkSelect value={hook.event} options={HOOK_EVENTS.map(([value, en, zh]) => ({ value, label: t(en, zh) }))} onChange={(event) => update({ event })} /></SettingsField>
        <SettingsField label={t("Action type", "动作类型")} anchor="hooks.kind"><ChoicePills value={hook.kind ?? "command"} options={[{ value: "command", label: "Shell" }, { value: "http", label: "HTTP" }]} onChange={(kind) => update({ kind })} /></SettingsField>
        <SettingsField label={t("Timeout", "超时")} anchor="hooks.timeout_ms" size="sm"><SkNumberInput value={hook.timeout_ms ?? 30_000} min={100} max={120000} unit="ms" onChange={(timeout_ms) => update({ timeout_ms })} /></SettingsField>
      </FieldGrid>
    </SettingsPanel>
    <HookActionFields key={selectedId} hook={hook} onChange={update} />
  </MasterDetail>;
}
