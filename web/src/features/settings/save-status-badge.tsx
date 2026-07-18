import { AlertCircle, CircleCheck, CircleDot } from "lucide-react";
import { useI18n } from "../i18n/use-i18n";

type SaveStatusBadgeProps = {
  dirty: boolean;
  saving: boolean;
  saveError: boolean;
  loaded: boolean;
};

/**
 * 渲染顶部保存状态徽标。
 *
 * @param props 脏标记、保存中、保存失败和已加载状态
 * @returns 保存状态徽标，配置未加载时返回 null
 */
export function SaveStatusBadge({ dirty, saving, saveError, loaded }: SaveStatusBadgeProps) {
  const { t } = useI18n();
  if (!loaded) return null;
  // 1. 保存失败优先展示，提示用户检查错误信息
  if (saveError && dirty) {
    return <span className="settings-status-badge failed"><AlertCircle size={13} /><span className="settings-status-badge-text">{t("Save failed", "保存失败")}</span></span>;
  }
  // 2. 保存中和未保存修改共用警示样式
  if (saving || dirty) {
    return <span className="settings-status-badge dirty"><CircleDot size={13} /><span className="settings-status-badge-text">{saving ? t("Saving", "正在保存") : t("Unsaved changes", "有未保存修改")}</span></span>;
  }
  // 3. 默认展示已保存
  return <span className="settings-status-badge saved"><CircleCheck size={13} /><span className="settings-status-badge-text">{t("Saved", "已保存")}</span></span>;
}
