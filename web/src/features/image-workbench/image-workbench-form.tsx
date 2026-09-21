import { ImagePlus, LoaderCircle, WandSparkles } from "lucide-react";
import type { ModelEndpointConfig } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { Select } from "../../shared/ui/select/select";
import { useI18n } from "../i18n/use-i18n";
import {
  IMAGE_ASPECT_RATIOS,
  imageResolutionsForAspectRatio,
  type ImageAspectRatio,
  type ImageGenerationSettings,
  type ImageResolution
} from "../chat/image-generation/image-generation-options";

type ImageWorkbenchFormProps = {
  prompt: string;
  endpoint: ModelEndpointConfig | null;
  endpoints: ModelEndpointConfig[];
  model: string;
  settings: ImageGenerationSettings;
  loading: boolean;
  submitting: boolean;
  onPromptChange: (value: string) => void;
  onEndpointChange: (id: string) => void;
  onModelChange: (value: string) => void;
  onSettingsChange: (settings: ImageGenerationSettings) => void;
  onSubmit: () => void;
};

/** 渲染独立生图工作台的提示词、端点和尺寸设置。 */
export function ImageWorkbenchForm(props: ImageWorkbenchFormProps) {
  const { t } = useI18n();
  const resolutions = imageResolutionsForAspectRatio(props.settings.aspectRatio);
  const endpointOptions = props.endpoints.map((item) => ({
    value: item.id,
    label: item.name || item.id,
    description: item.model || t("Model not set", "未填写模型")
  }));
  const modelOptions = (props.endpoint?.models ?? []).map((item) => ({ value: item, label: item }));

  return (
    <form className="image-workbench-form" onSubmit={(event) => { event.preventDefault(); props.onSubmit(); }}>
      <div className="image-workbench-form-heading">
        <span className="image-workbench-icon"><ImagePlus size={18} aria-hidden /></span>
        <div>
          <h1>{t("Image workbench", "生图工作台")}</h1>
          <p>{t("Create an image without mixing image controls into your conversation composer.", "独立生成图片，不占用聊天输入区。")}</p>
        </div>
      </div>
      <label className="image-workbench-prompt">
        <span>{t("Prompt", "提示词")}</span>
        <textarea
          value={props.prompt}
          onChange={(event) => props.onPromptChange(event.target.value)}
          placeholder={t("Describe the image you want to create", "描述你要生成的图片")}
          rows={6}
          disabled={props.submitting}
          autoFocus
        />
      </label>
      <div className="image-workbench-settings">
        <label>
          <span>{t("Image endpoint", "生图端点")}</span>
          <Select
            value={props.endpoint?.id ?? ""}
            options={endpointOptions.length > 0 ? endpointOptions : [{ value: "", label: t("No image model configured", "未配置生图模型") }]}
            disabled={props.loading || props.submitting || endpointOptions.length === 0}
            onChange={props.onEndpointChange}
            ariaLabel={t("Image endpoint", "生图端点")}
            menuPreferredWidth={280}
          />
        </label>
        {modelOptions.length > 0 && (
          <label>
            <span>{t("Model", "模型")}</span>
            <Select value={props.model} options={modelOptions} disabled={props.submitting} onChange={props.onModelChange} ariaLabel={t("Model", "模型")} />
          </label>
        )}
        <label>
          <span>{t("Aspect ratio", "画面比例")}</span>
          <Select
            value={props.settings.aspectRatio}
            options={IMAGE_ASPECT_RATIOS.map((item) => ({ value: item.value, label: item.label }))}
            disabled={props.submitting}
            onChange={(value) => {
              const aspectRatio = value as ImageAspectRatio;
              const nextResolution = imageResolutionsForAspectRatio(aspectRatio)[0]?.value ?? props.settings.resolution;
              props.onSettingsChange({ aspectRatio, resolution: nextResolution });
            }}
            ariaLabel={t("Aspect ratio", "画面比例")}
          />
        </label>
        <label>
          <span>{t("Resolution", "分辨率")}</span>
          <Select
            value={props.settings.resolution}
            options={resolutions.map((item) => ({ value: item.value, label: item.label }))}
            disabled={props.submitting}
            onChange={(value) => props.onSettingsChange({ ...props.settings, resolution: value as ImageResolution })}
            ariaLabel={t("Resolution", "分辨率")}
          />
        </label>
      </div>
      <Button variant="primary" type="submit" disabled={props.submitting || props.loading || !props.endpoint || !props.prompt.trim()}>
        {props.submitting ? <LoaderCircle size={15} className="image-workbench-spin" /> : <WandSparkles size={15} />}
        <span>{props.submitting ? t("Generating", "生成中") : t("Generate image", "生成图片")}</span>
      </Button>
    </form>
  );
}
