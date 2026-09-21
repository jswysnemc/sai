import { CircleAlert, LoaderCircle } from "lucide-react";
import { ImageGenerationToolView } from "../tool-renderers/image-generation-tool-view";
import { useI18n } from "../../i18n/use-i18n";
import "./image-generation-panel.css";

export type ImageGenerationResultState =
  | { status: "idle" }
  | { status: "loading"; prompt: string }
  | { status: "success"; prompt: string; output: string }
  | { status: "error"; prompt: string; message: string };

/** 在聊天时间线中展示直接生图请求的等待、成功和失败状态。 */
export function ImageGenerationResult({ result }: { result: ImageGenerationResultState }) {
  const { t } = useI18n();
  if (result.status === "idle") return null;
  return (
    <section className="chat-image-generation-result" aria-live="polite">
      <div className="chat-image-generation-prompt">
        <ImageGenerationMarker />
        <p>{result.prompt}</p>
      </div>
      {result.status === "loading" && (
        <div className="chat-image-generation-status" aria-busy="true">
          <LoaderCircle size={15} className="chat-image-generation-spin" aria-hidden />
          <span>{t("Generating image", "正在生成图片")}</span>
        </div>
      )}
      {result.status === "error" && (
        <div className="chat-image-generation-error" role="alert">
          <CircleAlert size={15} aria-hidden />
          <span>{result.message}</span>
        </div>
      )}
      {result.status === "success" && <ImageGenerationToolView output={result.output} />}
    </section>
  );
}

function ImageGenerationMarker() {
  return <span className="chat-image-generation-marker" aria-hidden />;
}
