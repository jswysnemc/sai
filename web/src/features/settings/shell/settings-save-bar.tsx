import { useEffect, useState } from "react";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { CircleAlert, CircleCheck, CircleDot, RotateCcw, Save } from "../../../shared/ui/icons";
import { Button } from "../../../shared/ui/button/button";
import { showsAppConfigSave } from "../settings-registry";
import type { SettingsSectionMeta } from "../settings-types";
import { useI18n } from "../../i18n/use-i18n";
import "./settings-save-bar.css";

type SettingsSaveBarProps = {
  meta: SettingsSectionMeta | undefined;
  dirty: boolean;
  saving: boolean;
  saveErrorMessage?: string;
  loaded: boolean;
  validationError?: string | null;
  onSave: () => void;
  onDiscard: () => void;
};

/**
 * 渲染内容区底部的悬浮保存操作栏。
 *
 * 桌面版下避免挤在窗口右上角系统控制键附近。
 * 在存在未保存修改、正在保存、校验失败或刚保存成功时浮现，平时保持隐蔽。
 *
 * @param props 分区元数据、草稿状态与保存、放弃回调
 * @returns 浮动保存操作栏
 */
export function SettingsSaveBar({
  meta,
  dirty,
  saving,
  saveErrorMessage,
  loaded,
  validationError,
  onSave,
  onDiscard
}: SettingsSaveBarProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const use = meta?.appConfig ?? "required";
  const [recentlySaved, setRecentlySaved] = useState(false);

  // 1. 【设置】【保存反馈】保存成功后短暂停留提示，随后自动收起
  useEffect(() => {
    if (!saving && !dirty && !saveErrorMessage && loaded) {
      setRecentlySaved(true);
      const timer = window.setTimeout(() => setRecentlySaved(false), 2400);
      return () => window.clearTimeout(timer);
    }
    setRecentlySaved(false);
  }, [dirty, saving, saveErrorMessage, loaded]);

  /**
   * 确认后放弃全部未保存修改。
   *
   * @returns 确认流程结束后返回
   */
  const handleDiscard = async () => {
    // 1. 【设置】【放弃修改】弹出确认对话框
    const confirmed = await confirm({
      title: t("Discard unsaved changes", "放弃未保存的修改"),
      description: t(
        "All changes made since the last save are reverted to the saved configuration.",
        "自上次保存以来的全部修改将恢复为已保存的配置。"
      ),
      confirmLabel: t("Discard changes", "放弃修改"),
      danger: true
    });
    // 2. 【设置】【放弃修改】确认后调用重置方法
    if (confirmed) onDiscard();
  };

  const shouldShow = showsAppConfigSave(use, dirty) && (dirty || saving || Boolean(saveErrorMessage) || Boolean(validationError) || recentlySaved);

  if (!shouldShow) return null;

  return (
    <aside className="settings-save-dock" role="region" aria-label={t("Save changes", "保存修改")}>
      <div className="settings-save-dock-state">
        {validationError ? (
          <span className="settings-save-dock-state is-failed" role="alert" title={validationError}>
            <CircleAlert size={14} />
            <span>{t("Fix JSON before saving", "请先修正 JSON")}</span>
          </span>
        ) : (
          <SaveDockState
            dirty={dirty}
            saving={saving}
            errorMessage={saveErrorMessage}
            recentlySaved={recentlySaved}
          />
        )}
      </div>
      <div className="settings-save-dock-actions">
        {dirty && !saving && (
          <Button variant="ghost" size="small" onClick={() => void handleDiscard()}>
            <RotateCcw size={14} />
            <span>{t("Discard", "放弃")}</span>
          </Button>
        )}
        <Button
          variant="primary"
          size="small"
          onClick={onSave}
          disabled={!loaded || !dirty || saving || Boolean(validationError)}
          title={t("Save changes (Ctrl+S)", "保存修改（Ctrl+S）")}
        >
          <Save size={14} />
          <span>{saving ? t("Saving", "正在保存") : t("Save", "保存")}</span>
        </Button>
      </div>
    </aside>
  );
}

type SaveDockStateProps = {
  dirty: boolean;
  saving: boolean;
  errorMessage?: string;
  recentlySaved: boolean;
};

/**
 * 渲染浮动栏状态文案与图标。
 *
 * @param props 草稿状态、保存中、错误与最近保存标记
 * @returns 状态展示
 */
function SaveDockState({ dirty, saving, errorMessage, recentlySaved }: SaveDockStateProps) {
  const { t } = useI18n();

  // 1. 【设置】【状态展示】保存失败分支
  if (errorMessage) {
    return (
      <span className="settings-save-dock-state is-failed" title={errorMessage} role="alert">
        <CircleAlert size={14} />
        <span>{t("Save failed", "保存失败")}</span>
      </span>
    );
  }

  // 2. 【设置】【状态展示】保存中或修改待保存分支
  if (saving || dirty) {
    return (
      <span className="settings-save-dock-state is-dirty">
        <CircleDot size={14} />
        <span>{saving ? t("Saving", "正在保存") : t("Unsaved changes", "有未保存的修改")}</span>
      </span>
    );
  }

  // 3. 【设置】【状态展示】保存完成反馈分支
  if (recentlySaved) {
    return (
      <span className="settings-save-dock-state is-saved">
        <CircleCheck size={14} />
        <span>{t("Saved", "已保存")}</span>
      </span>
    );
  }

  return null;
}
