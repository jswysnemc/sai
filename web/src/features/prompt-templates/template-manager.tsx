import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ActionMenu, type ActionMenuItem } from "../../shared/ui/menu/action-menu";
import { BookOpen, MoreHorizontal, Pencil, Plus, Trash2 } from "../../shared/ui/icons";
import { Button } from "../../shared/ui/button/button";
import { Modal } from "../../shared/ui/dialog/modal";
import { TextInput } from "../../shared/ui/form/text-input";
import { TextArea } from "../../shared/ui/form/text-area";
import { useI18n } from "../i18n/use-i18n";
import { templateApi, templateKey, type InputTemplate, type TemplateScope } from "./template-client";

const QUICK_LABELS: Record<TemplateScope, Record<string, string>> = {
  chat: { "tpl-explain": "了解项目", "tpl-review": "审阅变更", "tpl-test": "规划测试" },
  image: { "tpl-photo": "摄影", "tpl-product": "产品图", "tpl-illustration": "插画" }
};
const QUICK_CONTENT: Record<TemplateScope, Record<string, string>> = {
  chat: {
    "tpl-explain": "请解释当前项目的结构、主要模块、入口和关键工作流，并指出继续开发前需要了解的约束。",
    "tpl-review": "请审阅当前变更，重点检查正确性、边界条件、安全性和性能。按严重程度列出问题，并给出修复建议。",
    "tpl-test": "请为当前功能规划测试，覆盖正常流程、边界条件和错误处理，优先验证用户可观察的行为。"
  },
  image: {
    "tpl-photo": "生成一张写实摄影作品。\n主体：[描述主体]\n场景：[环境与背景]\n构图：[景别与视角]\n光线：[光源与氛围]\n要求：自然纹理，准确透视，不添加文字或水印。",
    "tpl-product": "生成一张商业产品摄影图。\n产品：[产品外观与材质]\n背景：[背景色与场景]\n构图：突出产品，保持轮廓清晰，预留适量留白。\n光线：柔和棚拍光，真实阴影与反射。",
    "tpl-illustration": "创作一幅插画。\n主题：[画面内容]\n风格：[插画风格]\n色彩：[主色与辅助色]\n构图：[主体位置与层次]\n氛围：[情绪与光线]。"
  }
};

/**
 * 渲染输入区模板入口、快捷模板和模板管理弹窗。
 *
 * @param props 场景、禁用状态和应用模板回调
 * @returns 输入区模板控件
 */
export function TemplateManager({ scope, disabled, onApply }: {
  scope: TemplateScope;
  disabled: boolean;
  onApply: (content: string) => void;
}) {
  const { t } = useI18n();
  const cache = useQueryClient();
  const [menuOpen, setMenuOpen] = useState(false);
  const [open, setOpen] = useState(false);
  const [current, setCurrent] = useState<InputTemplate | null>(null);
  const [name, setName] = useState("");
  const [content, setContent] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [deleting, setDeleting] = useState(false);
  const templates = useQuery({
    queryKey: templateKey(scope),
    queryFn: () => templateApi.list(scope),
    enabled: menuOpen || open,
    staleTime: 30_000
  });
  const quickTemplates = useMemo(() => {
    const labels = QUICK_LABELS[scope];
    return Object.entries(labels).map(([name, label]) => ({ name, label, content: QUICK_CONTENT[scope][name] ?? "" }));
  }, [scope]);

  /** 载入模板或清空表单；参数为模板，返回空值。 */
  function select(item: InputTemplate | null, confirmDelete = false) {
    setCurrent(item);
    setName(item?.name ?? "");
    setContent(item?.content ?? "");
    setError("");
    setDeleting(confirmDelete);
  }

  /** 打开模板管理弹窗；参数为要编辑的模板，返回空值。 */
  function openEditor(item: InputTemplate | null = null, confirmDelete = false) {
    select(item, confirmDelete);
    setOpen(true);
  }

  /** 将选中的模板正文替换到输入区；参数为模板，返回空值。 */
  function apply(item: InputTemplate) {
    onApply(item.content);
  }

  /** 保存或删除当前模板；参数为动作，返回完成状态。 */
  async function mutate(remove = false) {
    setBusy(true);
    setError("");
    try {
      if (remove && current) await templateApi.remove(scope, current.name);
      else await templateApi.save(scope, { name, content }, current?.builtin ? undefined : current?.name);
      await cache.invalidateQueries({ queryKey: templateKey(scope) });
      select(null);
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
    } finally {
      setBusy(false);
    }
  }

  const valid = /^[A-Za-z0-9_-]{1,64}$/.test(name)
    && Boolean(content.trim())
    && new TextEncoder().encode(content).length <= 65536;
  const menuItems = useMemo<ActionMenuItem[]>(() => {
    const items: ActionMenuItem[] = (templates.data ?? []).map((item) => ({
      id: `apply-${item.name}`,
      label: `${t("Apply", "应用")} /${item.name}`,
      icon: <BookOpen size={14} />,
      onSelect: () => apply(item)
    }));
    if (templates.isLoading) {
      items.push({ id: "loading", label: t("Loading templates…", "正在加载模板…"), disabled: true, onSelect: () => undefined });
    }
    items.push({
      id: "new",
      label: t("New template", "新建模板"),
      icon: <Plus size={14} />,
      separator: true,
      onSelect: () => openEditor()
    });
    for (const item of templates.data ?? []) {
      if (!item.builtin) {
        items.push({
          id: `edit-${item.name}`,
          label: `${t("Edit", "编辑")} /${item.name}`,
          icon: <Pencil size={14} />,
          onSelect: () => openEditor(item)
        });
        items.push({
          id: `delete-${item.name}`,
          label: `${t("Delete", "删除")} /${item.name}`,
          icon: <Trash2 size={14} />,
          danger: true,
          onSelect: () => openEditor(item, true)
        });
      }
    }
    return items;
  }, [t, templates.data, templates.isLoading]);

  return <>
    <div className="composer-template-trigger">
      <ActionMenu
        label={t("Prompt templates", "提示词模板")}
        trigger={<><BookOpen size={14} />{t("Templates", "模板")}</>}
        triggerSize="small"
        triggerClassName="composer-template-button"
        items={menuItems}
        disabled={disabled}
        onOpenChange={setMenuOpen}
      />
    </div>
    <div className="composer-template-suggestions" aria-label={t("Quick prompt templates", "快捷提示词模板")}>
      {quickTemplates.map(({ name, label, content }) => {
        const item = templates.data?.find((template) => template.name === name);
        return <Button key={name} variant="secondary" size="small" disabled={disabled} onClick={() => apply(item ?? { name, content, builtin: true })}>
          <BookOpen size={12} />{t(label, label)}
        </Button>;
      })}
    </div>
    {open && <Modal
      open
      title={scope === "image" ? t("Image prompt templates", "绘图提示词模板") : t("Chat prompt templates", "普通聊天提示词模板")}
      description={t("Choose a template to apply, or create and manage your own templates.", "选择模板应用到输入框，也可以新建和管理自定义模板。")}
      size="large"
      onClose={() => { if (!busy) setOpen(false); }}
      footer={<>
        <Button disabled={busy} onClick={() => setOpen(false)}>{t("Close", "关闭")}</Button>
        <Button variant="primary" disabled={busy || !valid || (current?.builtin === true && name === current.name)} onClick={() => void mutate()}>
          {busy ? t("Saving…", "正在保存…") : current?.builtin ? t("Save as copy", "另存为副本") : t("Save", "保存")}
        </Button>
      </>}
    >
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
          {current && !current.builtin && <Button variant="ghost-danger" disabled={busy} onClick={() => deleting ? void mutate(true) : setDeleting(true)}>
            {deleting ? t("Confirm deletion", "确认删除") : t("Delete template", "删除模板")}
          </Button>}
          {error && <p role="alert" className="break-words">{error}</p>}
        </div>
      </div>
    </Modal>}
  </>;
}
