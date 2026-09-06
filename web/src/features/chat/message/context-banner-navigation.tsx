import { ArrowDownToLine, ArrowUpToLine, X } from "lucide-react";
import { useState } from "react";
import { Button } from "../../../shared/ui/button/button";
import { Select } from "../../../shared/ui/select/select";
import { useI18n } from "../../i18n/use-i18n";
import "./context-banner-navigation.css";

/**
 * 在长上下文中持续提供章节、顶部、底部与关闭导航。
 * @param props 章节列表和各导航操作
 * @returns 紧凑的粘性导航条
 */
export function ContextBannerNavigation({ sections, onSection, onTop, onBottom, onClose }: {
  sections: { id: string; label: string }[];
  onSection: (id: string) => void;
  onTop: () => void;
  onBottom: () => void;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const [section, setSection] = useState("");
  return <nav className="context-banner-navigation" aria-label={t("Context navigation", "上下文导航")}>
    <Select value={section} options={[{ value: "", label: t("Sections", "内容目录") }, ...sections.map((item) => ({ value: item.id, label: item.label }))]} ariaLabel={t("Navigate context section", "跳转上下文章节")} menuPreferredWidth={260} onChange={(id) => { setSection(id); if (id) onSection(id); }} />
    <div className="context-banner-navigation-actions">
      <Button variant="ghost" size="small" onClick={onTop} aria-label={t("Go to context top", "前往上下文顶部")} title={t("Top", "顶部")}><ArrowUpToLine size={14} /><span className="hidden sm:inline">{t("Top", "顶部")}</span></Button>
      <Button variant="ghost" size="small" onClick={onBottom} aria-label={t("Go to context bottom", "前往上下文底部")} title={t("Bottom", "底部")}><ArrowDownToLine size={14} /><span className="hidden sm:inline">{t("Bottom", "底部")}</span></Button>
      <Button variant="ghost" size="small" onClick={onClose} aria-label={t("Close context", "关闭上下文")} title={t("Close", "关闭")}><X size={14} /><span className="hidden sm:inline">{t("Close", "关闭")}</span></Button>
    </div>
  </nav>;
}
