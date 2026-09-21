import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import { useI18n } from "../i18n/use-i18n";
import { ImageGenerationResult, type ImageGenerationResultState } from "../chat/image-generation/image-generation-result";
import { DEFAULT_IMAGE_GENERATION_SETTINGS, type ImageGenerationSettings } from "../chat/image-generation/image-generation-options";
import { useImageGenerationModel } from "../chat/image-generation/use-image-generation-model";
import { ImageWorkbenchForm } from "./image-workbench-form";
import "./image-workbench-page.css";

/** 渲染独立生图工作台页面。 */
export function ImageWorkbenchPage() {
  const { t } = useI18n();
  const sessions = useQuery({ queryKey: ["sessions"], queryFn: api.sessions.list });
  const activeSession = sessions.data?.find((session) => session.active);
  const imageModel = useImageGenerationModel(activeSession?.id);
  const [prompt, setPrompt] = useState("");
  const [model, setModel] = useState("");
  const [settings, setSettings] = useState<ImageGenerationSettings>(DEFAULT_IMAGE_GENERATION_SETTINGS);
  const [submitting, setSubmitting] = useState(false);
  const [result, setResult] = useState<ImageGenerationResultState>({ status: "idle" });

  useEffect(() => {
    setModel(imageModel.selection?.model ?? "");
  }, [imageModel.selection?.id, imageModel.selection?.model]);

  const submit = async () => {
    const value = prompt.trim();
    if (!value || !imageModel.selection) return;
    setSubmitting(true);
    setResult({ status: "loading", prompt: value });
    try {
      const response = await api.imageModels.generate({
        endpointId: imageModel.selection.id,
        model: model || imageModel.selection.model,
        prompt: value,
        aspectRatio: settings.aspectRatio,
        resolution: settings.resolution
      });
      setResult({ status: "success", prompt: value, output: JSON.stringify(response) });
      setPrompt("");
    } catch (error) {
      setResult({ status: "error", prompt: value, message: toDisplayError(error, "Image generation failed", "图片生成失败").message });
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <main className="image-workbench-page">
      <div className="image-workbench-layout">
        <ImageWorkbenchForm
          prompt={prompt}
          endpoint={imageModel.selection}
          endpoints={imageModel.endpoints}
          model={model}
          settings={settings}
          loading={imageModel.isLoading}
          submitting={submitting}
          onPromptChange={setPrompt}
          onEndpointChange={imageModel.selectEndpoint}
          onModelChange={setModel}
          onSettingsChange={setSettings}
          onSubmit={() => void submit()}
        />
        <section className="image-workbench-result" aria-live="polite">
          {result.status === "idle" ? (
            <div className="image-workbench-empty"><span>{t("Your generated image will appear here.", "生成的图片会显示在这里。")}</span></div>
          ) : <ImageGenerationResult result={result} />}
        </section>
      </div>
    </main>
  );
}
