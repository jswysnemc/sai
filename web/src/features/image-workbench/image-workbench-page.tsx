import { useEffect, useRef, useState } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import { useComposerAttachments } from "../chat/composer/use-composer-attachments";
import { DEFAULT_IMAGE_GENERATION_SETTINGS, type ImageGenerationSettings } from "../chat/image-generation/image-generation-options";
import { useImageGenerationModel } from "../chat/image-generation/use-image-generation-model";
import { useI18n } from "../i18n/use-i18n";
import { ImageWorkbenchComposer } from "./image-workbench-composer";
import { ImageWorkbenchSessions } from "./image-workbench-sessions";
import { ImageWorkbenchThread } from "./image-workbench-thread";
import {
  activeImageSession,
  appendImageTurn,
  createImageSession,
  imageFollowUpPrompt,
  loadImageWorkbenchStore,
  removeImageSession,
  saveImageWorkbenchStore,
  selectImageSession,
  updateImageTurn,
  type ImageWorkbenchStore,
  type ImageWorkbenchTurn
} from "./image-workbench-store";
import "./image-workbench-page.css";

/**
 * 渲染生图对话页：左侧会话，中间往返，底部输入框调整参数。
 *
 * @returns 生图工作台
 */
export function ImageWorkbenchPage() {
  const { t } = useI18n();
  const [store, setStore] = useState<ImageWorkbenchStore>(loadImageWorkbenchStore);
  const [prompt, setPrompt] = useState("");
  const [model, setModel] = useState("");
  const [settings, setSettings] = useState<ImageGenerationSettings>(DEFAULT_IMAGE_GENERATION_SETTINGS);
  const [submitting, setSubmitting] = useState(false);
  const [attachmentError, setAttachmentError] = useState<string | null>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const session = activeImageSession(store);
  const imageModel = useImageGenerationModel(session.id);
  const attachments = useComposerAttachments(session.id);

  useEffect(() => { saveImageWorkbenchStore(store); }, [store]);
  useEffect(() => { setModel(imageModel.selection?.model ?? ""); }, [imageModel.selection?.id, imageModel.selection?.model]);
  useEffect(() => {
    scroller.current?.scrollTo({ top: scroller.current.scrollHeight });
  }, [session.turns.length, session.turns.at(-1)?.status]);

  /**
   * 提交当前草稿，并把它作为新的一轮放进对话。
   */
  const addImages = async (files: File[], selectionStart: number, selectionEnd: number) => {
    setAttachmentError(null);
    try {
      return await attachments.addFiles(files, selectionStart, selectionEnd);
    } catch (error) {
      setAttachmentError(toDisplayError(error, "Failed to add image", "添加图片失败").message);
      return undefined;
    }
  };

  const submit = async () => {
    const typed = prompt.trim();
    const value = typed || (attachments.attachments.length > 0 ? t("Continue from the attached images", "根据附图继续") : "");
    if (!value || !imageModel.selection || submitting) return;
    const previous = session.turns.filter((item) => item.status !== "error").map((item) => item.prompt);
    const referenceImages = attachments.attachments.map((item) => item.dataUrl);
    const turn: ImageWorkbenchTurn = {
      id: `turn_${Date.now()}`,
      prompt: value,
      status: "loading",
      aspectRatio: settings.aspectRatio,
      resolution: settings.resolution,
      model: model || imageModel.selection.model,
      createdAt: new Date().toISOString(),
      attachments: attachments.attachments.map((item) => ({ name: item.name, dataUrl: item.dataUrl }))
    };
    setStore((current) => appendImageTurn(current, turn));
    setPrompt("");
    attachments.clearAttachments();
    setSubmitting(true);
    try {
      const response = await api.imageModels.generate({
        endpointId: imageModel.selection.id,
        model: turn.model,
        prompt: imageFollowUpPrompt(previous, value),
        aspectRatio: settings.aspectRatio,
        resolution: settings.resolution,
        images: referenceImages
      });
      setStore((current) => updateImageTurn(current, turn.id, { status: "success", output: JSON.stringify(response) }));
    } catch (error) {
      setStore((current) => updateImageTurn(current, turn.id, {
        status: "error",
        error: toDisplayError(error, "Image generation failed", "图片生成失败").message
      }));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <main className="image-workbench-page">
      <ImageWorkbenchSessions
        sessions={store.sessions}
        activeId={store.activeId}
        onSelect={(id) => { setPrompt(""); setStore((current) => selectImageSession(current, id)); }}
        onCreate={() => { setPrompt(""); setStore((current) => createImageSession(current)); }}
        onRemove={(id) => setStore((current) => removeImageSession(current, id))}
      />
      <section className={`image-workbench-main${session.turns.length === 0 ? " is-empty" : ""}`}>
        <div className="image-thread-scroll" ref={scroller}>
          {session.turns.length === 0 ? (
            <div className="image-empty">
              <h1>{t("What should we draw?", "想生成什么样的图片？")}</h1>
              <p>{t("Each session keeps its own prompts and images.", "每个会话各自保留提示词和图片。")}</p>
            </div>
          ) : <ImageWorkbenchThread turns={session.turns} />}
        </div>
        {attachmentError && <p className="image-composer-error" role="alert">{attachmentError}</p>}
        <ImageWorkbenchComposer
          value={prompt}
          history={session.turns.map((item) => item.prompt)}
          endpoint={imageModel.selection}
          endpoints={imageModel.endpoints}
          model={model}
          settings={settings}
          loading={imageModel.isLoading}
          submitting={submitting}
          attachments={attachments.attachments}
          onChange={setPrompt}
          onPasteImages={addImages}
          onRemoveAttachment={attachments.removeAttachment}
          onEndpointChange={imageModel.selectEndpoint}
          onModelChange={setModel}
          onSettingsChange={setSettings}
          onSubmit={() => void submit()}
        />
      </section>
    </main>
  );
}
