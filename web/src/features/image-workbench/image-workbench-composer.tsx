import { ArrowUp, Loader2, Plus } from "lucide-react";
import { useEffect, useRef, useState, type ChangeEvent } from "react";
import type { ModelEndpointConfig } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { Select } from "../../shared/ui/select/select";
import { ComposerSurface } from "../chat/composer/composer-surface";
import type { ComposerAttachment } from "../chat/composer/use-composer-attachments";
import {
  IMAGE_ASPECT_RATIOS,
  imageResolutionsForAspectRatio,
  type ImageGenerationSettings
} from "../chat/image-generation/image-generation-options";
import { useI18n } from "../i18n/use-i18n";
import "../chat/chat-composer.css";

const MENU_WIDTH = { menuPreferredWidth: 280, menuMinimumWidth: 240, menuAlign: "left" as const };

type ImageWorkbenchComposerProps = {
  value: string;
  history: string[];
  endpoint: ModelEndpointConfig | null;
  model: string;
  settings: ImageGenerationSettings;
  loading: boolean;
  submitting: boolean;
  attachments: ComposerAttachment[];
  onChange: (value: string) => void;
  onPasteImages: (files: File[], selectionStart: number, selectionEnd: number) => Promise<number | undefined>;
  onRemoveAttachment: (id: number) => void;
  onModelChange: (value: string) => void;
  onSettingsChange: (settings: ImageGenerationSettings) => void;
  onSubmit: () => void;
};

type ImageMenu = "ratio" | "size";

/**
 * 渲染与代码对话相同的输入框，并在底栏调整生图参数。
 *
 * @param props 草稿、端点、尺寸和提交回调
 * @returns 生图输入区
 */
export function ImageWorkbenchComposer(props: ImageWorkbenchComposerProps) {
  const { t } = useI18n();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const [menu, setMenu] = useState<ImageMenu | null>(null);
  const resolutions = imageResolutionsForAspectRatio(props.settings.aspectRatio);
  const hasDraft = Boolean(props.value.trim() || props.attachments.length);
  const modelOptions = (props.endpoint?.models ?? []).map((item) => ({ value: item, label: item }));
  const disabled = props.submitting || props.loading || !props.endpoint;

  useEffect(() => {
    if (!menu) return;
    const close = (event: PointerEvent) => {
      if (event.target instanceof Node && menuRef.current?.contains(event.target)) return;
      setMenu(null);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setMenu(null);
    };
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", close);
      document.removeEventListener("keydown", onKey);
    };
  }, [menu]);

  /**
   * 把文件选择器里的图片交给输入区。
   *
   * @param event 文件选择事件
   */
  const addUploadedFiles = (event: ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(event.target.files ?? []);
    event.target.value = "";
    if (files.length) void props.onPasteImages(files, props.value.length, props.value.length);
  };

  return (
    <div className="composer-shell">
      <ComposerSurface
        variant="full"
        className="composer"
        value={props.value}
        historyEntries={props.history}
        disabled={props.submitting}
        submitDisabled={disabled || !hasDraft}
        autoFocus
        placeholder={props.endpoint ? t("Describe the image you want to create", "描述你要生成的图片") : t("Configure an image model first", "请先配置生图模型")}
        attachments={props.attachments}
        onChange={props.onChange}
        onPasteImages={props.onPasteImages}
        onRemoveAttachment={props.onRemoveAttachment}
        onSubmit={props.onSubmit}
      >
        <div className="composer-footer">
          <input ref={fileInputRef} type="file" accept="image/*" multiple hidden onChange={addUploadedFiles} />
          <Button variant="ghost" size="icon" className="composer-attach" disabled={props.submitting} title={t("Attach images", "添加图片")} aria-label={t("Add images", "添加图片")} onClick={() => fileInputRef.current?.click()}><Plus size={18} /></Button>
          <div className="composer-model-controls">
            {modelOptions.length > 0 && (
              <div className="composer-mode">
                <Select value={props.model} options={modelOptions} disabled={props.submitting} onChange={props.onModelChange} ariaLabel={t("Model", "模型")} {...MENU_WIDTH} />
              </div>
            )}
            <div className="image-param-menus" ref={menuRef}>
              <div className="image-option">
                <button type="button" aria-expanded={menu === "ratio"} aria-label={t("Aspect ratio", "画面比例")} disabled={props.submitting} onClick={() => setMenu((current) => current === "ratio" ? null : "ratio")}>
                  {props.settings.aspectRatio}
                </button>
                {menu === "ratio" && (
                  <div className="image-option-menu" role="listbox" aria-label={t("Aspect ratio", "画面比例")}>
                    {IMAGE_ASPECT_RATIOS.map((item) => (
                      <button
                        key={item.value}
                        type="button"
                        role="option"
                        aria-selected={item.value === props.settings.aspectRatio}
                        className={item.value === props.settings.aspectRatio ? "is-active" : undefined}
                        onClick={() => {
                          const aspectRatio = item.value;
                          const resolution = imageResolutionsForAspectRatio(aspectRatio)[0]?.value ?? props.settings.resolution;
                          props.onSettingsChange({ aspectRatio, resolution });
                          setMenu(null);
                        }}
                      >
                        {item.label}
                      </button>
                    ))}
                  </div>
                )}
              </div>
              <div className="image-option">
                {resolutions.length < 2 ? (
                  <span className="image-option-value">{props.settings.resolution.replace("x", "×")}</span>
                ) : (
                <button
                  type="button"
                  aria-expanded={menu === "size"}
                  aria-label={t("Resolution", "分辨率")}
                  disabled={props.submitting}
                  onClick={() => setMenu((current) => current === "size" ? null : "size")}
                >
                  {props.settings.resolution.replace("x", "×")}
                </button>
                )}
                {menu === "size" && (
                  <div className="image-option-menu" role="listbox" aria-label={t("Resolution", "分辨率")}>
                    {resolutions.map((item) => (
                      <button
                        key={item.value}
                        type="button"
                        role="option"
                        aria-selected={item.value === props.settings.resolution}
                        className={item.value === props.settings.resolution ? "is-active" : undefined}
                        onClick={() => {
                          props.onSettingsChange({ ...props.settings, resolution: item.value });
                          setMenu(null);
                        }}
                      >
                        {item.label}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            </div>
          </div>
          <div className="composer-actions">
            <Button variant="primary" size="icon" type="submit" className="composer-send" disabled={disabled || !hasDraft} aria-label={t("Generate image", "生成图片")} title={t("Generate image", "生成图片")}>
              {props.submitting ? <Loader2 size={16} className="composer-send-spin" /> : <ArrowUp size={18} />}
            </Button>
          </div>
        </div>
      </ComposerSurface>
    </div>
  );
}
