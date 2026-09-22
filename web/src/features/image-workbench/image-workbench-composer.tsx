import { ArrowUp, Loader2, Plus } from "lucide-react";
import { useRef, type ChangeEvent } from "react";
import { useNavigate } from "react-router-dom";
import type { ModelEndpointConfig } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { Select } from "../../shared/ui/select/select";
import { ComposerSurface } from "../chat/composer/composer-surface";
import type { ComposerAttachment } from "../chat/composer/use-composer-attachments";
import {
  IMAGE_ASPECT_RATIOS,
  imageResolutionsForAspectRatio,
  type ImageAspectRatio,
  type ImageGenerationSettings,
  type ImageResolution
} from "../chat/image-generation/image-generation-options";
import { useI18n } from "../i18n/use-i18n";
import "../chat/chat-composer.css";

const MENU_WIDTH = { menuPreferredWidth: 280, menuMinimumWidth: 240, menuAlign: "left" as const };

type ImageWorkbenchComposerProps = {
  value: string;
  history: string[];
  endpoint: ModelEndpointConfig | null;
  endpoints: ModelEndpointConfig[];
  model: string;
  settings: ImageGenerationSettings;
  loading: boolean;
  submitting: boolean;
  attachments: ComposerAttachment[];
  onChange: (value: string) => void;
  onPasteImages: (files: File[], selectionStart: number, selectionEnd: number) => Promise<number | undefined>;
  onRemoveAttachment: (id: number) => void;
  onEndpointChange: (id: string) => void;
  onModelChange: (value: string) => void;
  onSettingsChange: (settings: ImageGenerationSettings) => void;
  onSubmit: () => void;
};

/**
 * 渲染与代码对话相同的输入框，并在底栏调整生图参数。
 *
 * @param props 草稿、端点、尺寸和提交回调
 * @returns 生图输入区
 */
export function ImageWorkbenchComposer(props: ImageWorkbenchComposerProps) {
  const { t } = useI18n();
  const navigate = useNavigate();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const resolutions = imageResolutionsForAspectRatio(props.settings.aspectRatio);
  const hasDraft = Boolean(props.value.trim() || props.attachments.length);
  const endpointOptions = props.endpoints.map((item) => ({
    value: item.id,
    label: item.name || item.id,
    description: item.model || t("Model not set", "未填写模型")
  }));
  const modelOptions = (props.endpoint?.models ?? []).map((item) => ({ value: item, label: item }));
  const disabled = props.submitting || props.loading || !props.endpoint;

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
            <div className="composer-mode">
              <Select
                value="image"
                options={[
                  { value: "image", label: t("Image", "生图"), description: t("Keep this image conversation", "留在生图对话") },
                  { value: "chat", label: t("Normal mode", "正常模式"), description: t("Return to the coding session", "回到代码会话") }
                ]}
                disabled={props.submitting}
                onChange={(value) => { if (value === "chat") navigate("/"); }}
                ariaLabel={t("Composer mode", "输入模式")}
                {...MENU_WIDTH}
              />
            </div>
            <div className="composer-mode">
              <Select
                value={props.endpoint?.id ?? ""}
                options={endpointOptions.length > 0 ? endpointOptions : [{ value: "", label: t("No image model", "未配置") }]}
                disabled={props.loading || props.submitting || endpointOptions.length === 0}
                onChange={props.onEndpointChange}
                ariaLabel={t("Image endpoint", "生图端点")}
                {...MENU_WIDTH}
              />
            </div>
            {modelOptions.length > 0 && (
              <div className="composer-mode">
                <Select value={props.model} options={modelOptions} disabled={props.submitting} onChange={props.onModelChange} ariaLabel={t("Model", "模型")} {...MENU_WIDTH} />
              </div>
            )}
            <div className="composer-mode">
              <Select
                value={props.settings.aspectRatio}
                options={IMAGE_ASPECT_RATIOS.map((item) => ({ value: item.value, label: item.label }))}
                disabled={props.submitting}
                onChange={(value) => {
                  const aspectRatio = value as ImageAspectRatio;
                  const resolution = imageResolutionsForAspectRatio(aspectRatio)[0]?.value ?? props.settings.resolution;
                  props.onSettingsChange({ aspectRatio, resolution });
                }}
                ariaLabel={t("Aspect ratio", "画面比例")}
                {...MENU_WIDTH}
              />
            </div>
            <div className="composer-mode">
              <Select
                value={props.settings.resolution}
                options={resolutions.map((item) => ({ value: item.value, label: item.label }))}
                disabled={props.submitting}
                onChange={(value) => props.onSettingsChange({ ...props.settings, resolution: value as ImageResolution })}
                ariaLabel={t("Resolution", "分辨率")}
                {...MENU_WIDTH}
              />
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
