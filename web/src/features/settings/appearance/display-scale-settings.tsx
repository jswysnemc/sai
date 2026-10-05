import { useState } from "react";
import { useI18n } from "../../i18n/use-i18n";
import {
  FONT_SCALE_OPTIONS,
  loadDisplayScale,
  PAGE_ZOOM_OPTIONS,
  saveDisplayScale
} from "../../theme/display-scale";
import { ChoicePills, SettingsField, SettingsPanel } from "../kit";

/**
 * 【Web 设置】【显示尺度】调整界面字号和页面缩放，立即生效并保存在当前浏览器。
 * @returns 字号与缩放面板
 */
export function DisplayScaleSettings() {
  const { t } = useI18n();
  const [scale, setScale] = useState(loadDisplayScale);

  /**
   * 更新一项尺度并写回本地存储。
   * @param patch 字号或缩放
   * @returns 无
   */
  const update = (patch: Partial<typeof scale>) => {
    const next = { ...scale, ...patch };
    setScale(next);
    saveDisplayScale(next);
  };

  return (
    <SettingsPanel
      title={t("Text and zoom", "字号与缩放")}
      description={t("Adjusts this browser only. Font size changes text; zoom scales the whole page.", "仅作用于当前浏览器。字号改变文字，缩放改变整页。")}
    >
      <SettingsField label={t("Font size", "界面字号")} anchor="appearance.fontScale" hint={t("100% is 16px on desktop.", "100% 对应桌面 16px。")}>
        <ChoicePills
          value={String(scale.fontScale)}
          options={FONT_SCALE_OPTIONS}
          onChange={(value) => update({ fontScale: Number(value) })}
          ariaLabel={t("Font size", "界面字号")}
        />
      </SettingsField>
      <SettingsField label={t("Page zoom", "页面缩放")} anchor="appearance.pageZoom" hint={t("Use this if controls feel too small after changing font size.", "字号调整后控件仍偏小时使用。")}>
        <ChoicePills
          value={String(scale.pageZoom)}
          options={PAGE_ZOOM_OPTIONS}
          onChange={(value) => update({ pageZoom: Number(value) })}
          ariaLabel={t("Page zoom", "页面缩放")}
        />
      </SettingsField>
    </SettingsPanel>
  );
}
