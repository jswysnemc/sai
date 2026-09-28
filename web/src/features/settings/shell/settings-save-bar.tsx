import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { CircleAlert, CircleCheck, CircleDot, RotateCcw, Save } from "../../../shared/ui/icons";
import { Button } from "../../../shared/ui/button/button";
import { showsAppConfigSave } from "../settings-registry";
import type { SettingsSectionMeta } from "../settings-types";
import { useI18n } from "../../i18n/use-i18n";

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
 * 渲染顶栏保存区：保存状态、放弃修改与保存按钮。
 *
 * required 分区常驻保存；其余分区只在全局草稿有修改时露出保存，
 * 平时显示注册表声明的保存方式（即时生效、本节保存、只读）。
 *
 * @param props 分区元数据、草稿状态与保存、放弃回调
 * @returns 保存区
 */
export function SettingsSaveBar({ meta, dirty, saving, saveErrorMessage, loaded, validationError, onSave, onDiscard }: SettingsSaveBarProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const use = meta?.appConfig ?? "required";

  /**
   * 确认后放弃全部未保存修改。
   *
   * @returns 确认流程结束后返回
   */
  const discard = async () => {
    const confirmed = await confirm({
      title: t("Discard unsaved changes", "放弃未保存的修改"),
      description: t(
        "All changes made since the last save are reverted to the saved configuration.",
        "自上次保存以来的全部修改将恢复为已保存的配置。"
      ),
      confirmLabel: t("Discard changes", "放弃修改"),
      danger: true
    });
    if (confirmed) onDiscard();
  };

  // 1. 不参与全局保存且没有待保存修改：只展示保存方式
  if (!showsAppConfigSave(use, dirty)) {
    return meta?.saveHintEn && meta.saveHintZh
      ? <span className="settings-save-hint">{t(meta.saveHintEn, meta.saveHintZh)}</span>
      : null;
  }

  return (
    <div className="settings-save-bar">
      {validationError ? <span className="settings-save-state is-failed" title={validationError} role="alert">{t("Fix JSON before saving", "请先修正 JSON")}</span> : loaded && <SaveState dirty={dirty} saving={saving} errorMessage={saveErrorMessage} />}
      {dirty && !saving && (
        <Button variant="ghost" size="small" onClick={() => void discard()}>
          <RotateCcw size={14} />
          <span className="settings-save-label">{t("Discard", "放弃")}</span>
        </Button>
      )}
      <Button
        variant="primary"
        size="small"
        className="settings-save-button"
        onClick={onSave}
        disabled={!loaded || !dirty || saving || Boolean(validationError)}
        title={t("Save changes (Ctrl+S)", "保存修改（Ctrl+S）")}
      >
        <Save size={14} />
        <span className="settings-save-label">{saving ? t("Saving", "正在保存") : t("Save", "保存")}</span>
      </Button>
    </div>
  );
}

type SaveStateProps = {
  dirty: boolean;
  saving: boolean;
  errorMessage?: string;
};

/**
 * 渲染保存状态文字：失败、保存中、有修改或已保存。
 *
 * @param props 草稿状态与错误信息
 * @returns 状态文字
 */
function SaveState({ dirty, saving, errorMessage }: SaveStateProps) {
  const { t } = useI18n();
  if (errorMessage) {
    return (
      <span className="settings-save-state is-failed" title={errorMessage} role="alert">
        <CircleAlert size={14} />
        <span>{t("Save failed", "保存失败")}</span>
      </span>
    );
  }
  if (saving || dirty) {
    return (
      <span className="settings-save-state is-dirty">
        <CircleDot size={14} />
        <span>{saving ? t("Saving", "正在保存") : t("Unsaved changes", "有未保存的修改")}</span>
      </span>
    );
  }
  return (
    <span className="settings-save-state">
      <CircleCheck size={14} />
      <span>{t("Saved", "已保存")}</span>
    </span>
  );
}
