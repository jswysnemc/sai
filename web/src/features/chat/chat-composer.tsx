import { ArrowUp, Loader2, Plus, Square } from "lucide-react";
import { useRef, type ChangeEvent } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../../api/client";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import { ComposerSurface } from "./composer/composer-surface";
import { resolveComposerAvailability } from "./composer-availability";
import { ComposerModelControls } from "./chat-composer/composer-model-controls";
import { ComposerContextFooter } from "./chat-composer/composer-context-footer";
import type { ChatComposerProps } from "./chat-composer/composer-types";
import "./chat-composer.css";

/**
 * 渲染任务输入、附件、模型选择和发送状态。
 * @param props 草稿、运行状态、会话配置和操作回调
 * @returns 保留队列与停止语义的聊天输入区
 */
export function ChatComposer(props: ChatComposerProps) {
  const { t } = useI18n();
  const engineStatus = useQuery({ queryKey: ["engine-status"], queryFn: api.config.engineStatus, staleTime: 60_000 });
  const externalEngine = engineStatus.data?.external === true ? engineStatus.data : null;
  const enginePending = engineStatus.isLoading && !engineStatus.data;
  const fileInputRef = useRef<HTMLInputElement>(null);
  const availability = resolveComposerAvailability({
    sessionAvailable: props.sessionAvailable,
    runActive: props.running,
    runStatus: props.runStatus,
    hasDraft: Boolean(props.value.trim()) || props.attachments.length > 0,
    submitBlocked: props.submitBlocked || props.submitting
  });

  /**
   * 将文件选择器中的图片加入当前草稿。
   * @param event 文件选择事件
   * @returns 无返回值
   */
  const handleFileChange = (event: ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(event.target.files ?? []);
    event.target.value = "";
    if (files.length) void props.onAddImages(files, props.value.length, props.value.length);
  };
  const placeholder = !props.sessionAvailable ? t("Select a session first", "请先选择会话")
    : props.running ? t("Add a follow-up. Enter to queue it.", "补充任务内容，Enter 加入队列")
      : t("Describe a task, or type @ to add context", "描述任务，或输入 @ 添加上下文");

  return (
    <div className="composer-shell">
      <ComposerSurface
        variant="full"
        className="composer"
        value={props.value}
        historyEntries={props.historyEntries}
        disabled={availability.inputDisabled}
        submitDisabled={availability.sendDisabled}
        placeholder={placeholder}
        attachments={props.attachments}
        onChange={props.onChange}
        onPasteImages={props.onAddImages}
        onRemoveAttachment={props.onRemoveAttachment}
        onSubmit={props.onSubmit}
      >
        <div className="composer-footer">
          <input ref={fileInputRef} type="file" accept="image/*" multiple onChange={handleFileChange} hidden />
          <Button variant="ghost" size="icon" className="composer-attach" onClick={() => fileInputRef.current?.click()} disabled={availability.inputDisabled} title={t("Attach images", "添加图片")} aria-label={t("Add images", "添加图片")}><Plus size={18} /></Button>
          <ComposerModelControls composer={props} engine={externalEngine} loading={enginePending} />
          <div className="composer-actions">
            {availability.showStop ? (
              <Button variant="primary" size="icon" className="composer-send stop" onClick={props.onStop} aria-label={t("Stop run", "停止运行")} title={t("Stop run", "停止运行")}><Square size={12} fill="currentColor" /></Button>
            ) : (
              <Button variant="primary" size="icon" type="submit" className="composer-send" disabled={availability.sendDisabled || props.submitting} aria-label={props.running ? t("Queue message", "排队发送") : t("Send message", "发送消息")} title={props.running ? t("Queue message", "排队发送") : t("Send message", "发送消息")}>
                {props.submitting ? <Loader2 size={16} className="composer-send-spin" /> : <ArrowUp size={18} />}
              </Button>
            )}
          </div>
        </div>
      </ComposerSurface>
      <ComposerContextFooter composer={props} showUsage={!externalEngine && !enginePending} />
    </div>
  );
}
