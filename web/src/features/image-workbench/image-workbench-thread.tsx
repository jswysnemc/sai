import { CircleAlert } from "lucide-react";
import { ImageGenerationToolView } from "../chat/tool-renderers/image-generation-tool-view";
import { ImageWorkbenchWaiting } from "./image-workbench-waiting";
import type { ImageWorkbenchTurn } from "./image-workbench-store";

type ImageWorkbenchThreadProps = {
  turns: ImageWorkbenchTurn[];
};

/**
 * 按发送顺序渲染提示词和生成结果。
 *
 * @param props 当前会话的轮次
 * @returns 对话流
 */
export function ImageWorkbenchThread({ turns }: ImageWorkbenchThreadProps) {
  return (
    <div className="image-thread">
      {turns.map((turn) => (
        <article className="image-turn" key={turn.id}>
          <div className="image-turn-user">
            <div>
              {turn.attachments && turn.attachments.length > 0 && (
                <div className="image-turn-attachments">
                  {turn.attachments.map((item) => <img key={item.dataUrl} src={item.dataUrl} alt={item.name} />)}
                </div>
              )}
              <p>{turn.prompt}</p>
              <small>{[turn.aspectRatio, turn.resolution.replace("x", " × "), turn.model].filter(Boolean).join(" · ")}</small>
            </div>
          </div>
          <div className="image-turn-result">
            {turn.status === "loading" && <ImageWorkbenchWaiting aspectRatio={turn.aspectRatio} />}
            {turn.status === "error" && (
              <p className="image-turn-error" role="alert"><CircleAlert size={15} aria-hidden /><span>{turn.error}</span></p>
            )}
            {turn.status === "success" && turn.output && <ImageGenerationToolView output={turn.output} />}
          </div>
        </article>
      ))}
    </div>
  );
}
