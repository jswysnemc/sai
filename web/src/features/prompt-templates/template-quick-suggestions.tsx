import { useQuery } from "@tanstack/react-query";
import { Button } from "../../shared/ui/button/button";
import { BookOpen, FileSearch, GitPullRequest, ListChecks } from "../../shared/ui/icons";
import { FOCUS_COMPOSER_EVENT } from "../chat/composer/composer-events";
import { useI18n } from "../i18n/use-i18n";
import { templateApi, templateKey, type TemplateScope } from "./template-client";
import { TemplateManager } from "./template-manager";

const QUICK_TEMPLATES = {
  chat: [
    { name: "tpl-explain", en: "Explore project", zh: "了解项目", icon: FileSearch },
    { name: "tpl-review", en: "Review changes", zh: "审阅变更", icon: GitPullRequest },
    { name: "tpl-test", en: "Plan tests", zh: "规划测试", icon: ListChecks }
  ],
  image: [
    { name: "tpl-photo", en: "Photography", zh: "摄影", icon: BookOpen },
    { name: "tpl-product", en: "Product image", zh: "产品图", icon: BookOpen },
    { name: "tpl-illustration", en: "Illustration", zh: "插画", icon: BookOpen }
  ]
};

/**
 * 展示空会话状态栏下方的快捷模板和更多菜单。
 * @param props 模板场景、禁用状态和草稿更新回调
 * @returns 只填入草稿、不自动发送的快捷入口
 */
export function TemplateQuickSuggestions({ scope, disabled, onApply }: {
  scope: TemplateScope;
  disabled: boolean;
  onApply: (content: string) => void;
}) {
  const { t } = useI18n();
  const templates = useQuery({ queryKey: templateKey(scope), queryFn: () => templateApi.list(scope), staleTime: 30_000 });

  /**
   * 填入服务端模板正文并将焦点移回输入框。
   * @param content 模板正文
   * @returns 无返回值
   */
  function apply(content: string) {
    onApply(content);
    window.requestAnimationFrame(() => window.dispatchEvent(new Event(FOCUS_COMPOSER_EVENT)));
  }

  return <div className="composer-template-suggestions gap-x-1.5 gap-y-2 sm:gap-y-1.5" aria-label={t("Quick prompt templates", "快捷提示词模板")}>
    {QUICK_TEMPLATES[scope].map(({ name, en, zh, icon: Icon }) => {
      const item = templates.data?.find((template) => template.name === name);
      return <Button key={name} variant="ghost" size="small" disabled={disabled || !item} onClick={() => item && apply(item.content)}>
        <Icon size={14} />{t(en, zh)}
      </Button>;
    })}
    <TemplateManager scope={scope} disabled={disabled} onApply={onApply} placement="inline" />
  </div>;
}
