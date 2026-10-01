import { useEffect, useState } from "react";
import { Select } from "../../shared/ui/select/select";
import { TextInput } from "../../shared/ui/form/text-input";
import { useI18n } from "../i18n/use-i18n";
import { RESPONSIVE_PRESETS, ZOOM_OPTIONS, type BrowserZoom, type ResponsiveViewport } from "./browser-responsive";

type BrowserResponsiveBarProps = {
  viewport: ResponsiveViewport;
  onSize: (size: { width: number; height: number }) => void;
  onZoom: (zoom: BrowserZoom) => void;
};

/** 宽高输入允许的范围，与服务端限制一致。 */
const LIMITS = { width: [320, 3840], height: [240, 2160] } as const;

/**
 * 自由尺寸工具条：设备预设、宽高输入、宽高互换与缩放档位。
 *
 * 宽高在失焦或回车时提交，输入过程中不会反复调整远端页面。
 *
 * @param props 当前尺寸与修改回调
 * @returns 工具条
 */
export function BrowserResponsiveBar({ viewport, onSize, onZoom }: BrowserResponsiveBarProps) {
  const { t } = useI18n();
  const [width, setWidth] = useState(String(viewport.size.width));
  const [height, setHeight] = useState(String(viewport.size.height));
  const [invalid, setInvalid] = useState(false);

  useEffect(() => {
    setWidth(String(viewport.size.width));
    setHeight(String(viewport.size.height));
  }, [viewport.size.width, viewport.size.height]);

  /** 校验并提交宽高。 */
  const commit = () => {
    const next = { width: Number(width), height: Number(height) };
    const valid = Number.isInteger(next.width) && Number.isInteger(next.height)
      && next.width >= LIMITS.width[0] && next.width <= LIMITS.width[1]
      && next.height >= LIMITS.height[0] && next.height <= LIMITS.height[1];
    setInvalid(!valid);
    if (valid) onSize(next);
  };

  const preset = RESPONSIVE_PRESETS.find((item) => item.width === viewport.size.width && item.height === viewport.size.height);
  const presetOptions = [
    ...RESPONSIVE_PRESETS.map((item) => ({ value: item.id, label: `${t(item.labelEn, item.labelZh)} ${item.width}×${item.height}` })),
    ...(preset ? [] : [{ value: "custom", label: t("Custom", "自定义") }])
  ];
  const zoomOptions = ZOOM_OPTIONS.map((zoom) => ({
    value: String(zoom),
    label: zoom === "fit" ? t("Fit to pane", "适应面板") : `${Math.round(zoom * 100)}%`
  }));

  return (
    <div className="browser-responsive-bar" role="group" aria-label={t("Free size viewport", "自由尺寸浏览器视口")}>
      <Select
        value={preset?.id ?? "custom"}
        options={presetOptions}
        ariaLabel={t("Device preset", "设备预设")}
        onChange={(id) => {
          const next = RESPONSIVE_PRESETS.find((item) => item.id === id);
          if (next) onSize({ width: next.width, height: next.height });
        }}
      />
      <form
        className="browser-responsive-size"
        onSubmit={(event) => {
          event.preventDefault();
          commit();
        }}
      >
        <TextInput
          value={width}
          inputMode="numeric"
          aria-label={t("Viewport width", "浏览器视口宽度")}
          aria-invalid={invalid}
          onChange={(event) => setWidth(event.target.value.replace(/\D/g, ""))}
          onBlur={commit}
        />
        <button
          type="button"
          className="browser-responsive-swap"
          aria-label={t("Swap width and height", "交换宽高")}
          title={t("Swap width and height", "交换宽高")}
          onClick={() => onSize({ width: viewport.size.height, height: viewport.size.width })}
        >
          ×
        </button>
        <TextInput
          value={height}
          inputMode="numeric"
          aria-label={t("Viewport height", "浏览器视口高度")}
          aria-invalid={invalid}
          onChange={(event) => setHeight(event.target.value.replace(/\D/g, ""))}
          onBlur={commit}
        />
      </form>
      <Select
        value={String(viewport.zoom)}
        options={zoomOptions}
        ariaLabel={t("Preview zoom", "浏览器预览缩放")}
        onChange={(value) => onZoom(value === "fit" ? "fit" : (Number(value) as BrowserZoom))}
      />
      {invalid && (
        <span className="browser-responsive-error" role="alert">
          {t("Width 320–3840, height 240–2160", "宽 320–3840，高 240–2160")}
        </span>
      )}
    </div>
  );
}
