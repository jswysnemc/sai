import { AlertCircle, CircleCheck, CircleDot } from "lucide-react";

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
  if (!loaded) return null;
  // 1. 保存失败优先展示，提示用户检查错误信息
  if (saveError && dirty) {
    return <span className="settings-status-badge failed"><AlertCircle size={13} />保存失败</span>;
  }
  // 2. 保存中和未保存修改共用警示样式
  if (saving || dirty) {
    return <span className="settings-status-badge dirty"><CircleDot size={13} />{saving ? "正在保存" : "有未保存修改"}</span>;
  }
  // 3. 默认展示已保存
  return <span className="settings-status-badge saved"><CircleCheck size={13} />已保存</span>;
}
