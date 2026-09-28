import { useRef, type ChangeEvent, type ComponentProps } from "react";
import { ArrowUp, Loader2, Plus, Square } from "../../shared/ui/icons";
import { Button } from "../../shared/ui/button/button";
import { Select } from "../../shared/ui/select/select";
import type { RunMode } from "../../api/contracts";
import { ComposerSurface } from "../chat/composer/composer-surface";
import { ModelThinkingSelector } from "../chat/model-thinking-selector";
import { createRunModeOptions } from "../permission/run-mode-options";
import { useI18n } from "../i18n/use-i18n";

type Props = Pick<ComponentProps<typeof ComposerSurface>, "value" | "attachments" | "onChange" | "onPasteImages" | "onRemoveAttachment" | "onSubmit"> & {
  running: boolean;
  submitting: boolean;
  mode: RunMode;
  onModeChange: (mode: RunMode) => void;
  onStop: () => void;
  model: ComponentProps<typeof ModelThinkingSelector>;
};

/**
 * 组合旁路输入区，沿用主会话的编辑器、附件按钮及框外权限栏。
 * @param props 草稿、附件、模型偏好和运行操作
 * @returns 支持文字与纯图片提交的旁路编辑器
 */
export function SideConversationComposer(props: Props) {
  const { t } = useI18n();
  const fileInput = useRef<HTMLInputElement>(null);
  const disabled = props.running || props.submitting;
  const canSend = Boolean(props.value.trim()) || Boolean(props.attachments?.length);

  /** 将文件选择交给附件处理；参数为选择事件，返回无。 */
  const selectImages = (event: ChangeEvent<HTMLInputElement>): void => {
    const files = Array.from(event.target.files ?? []);
    event.target.value = "";
    if (files.length) void props.onPasteImages?.(files, props.value.length, props.value.length);
  };

  return (
    <div className="composer-shell side-conversation-composer">
      <ComposerSurface variant="full" className="composer" value={props.value} historyEntries={[]}
        disabled={disabled} submitDisabled={disabled || !canSend} respondToGlobalFocus={false}
        placeholder={t("Ask a question about this response", "针对这条回复提问")}
        attachments={props.attachments} onChange={props.onChange} onPasteImages={props.onPasteImages}
        onRemoveAttachment={props.onRemoveAttachment} onSubmit={props.onSubmit}>
        <div className="composer-footer">
          <input ref={fileInput} type="file" accept="image/*" multiple hidden onChange={selectImages} />
          <Button variant="ghost" size="icon" className="composer-attach" disabled={disabled}
            aria-label={t("Add images", "添加图片")} title={t("Add images", "添加图片")}
            onClick={() => fileInput.current?.click()}><Plus size={16} /></Button>
          <div className="composer-model-controls"><ModelThinkingSelector {...props.model} disabled={disabled} /></div>
          <div className="composer-actions">
            {props.running ? (
              <Button variant="primary" size="icon" className="composer-send stop" onClick={props.onStop}
                aria-label={t("Stop run", "停止运行")} title={t("Stop run", "停止运行")}><Square size={12} fill="currentColor" /></Button>
            ) : (
              <Button variant="primary" size="icon" type="submit" className="composer-send" disabled={!canSend || props.submitting}
                aria-label={t("Send message", "发送消息")} title={t("Send message", "发送消息")}>
                {props.submitting ? <Loader2 size={16} className="composer-send-spin" /> : <ArrowUp size={16} />}
              </Button>
            )}
          </div>
        </div>
      </ComposerSurface>
      <div className="composer-context-footer">
        <div className="composer-context-options"><div className="composer-mode">
          <Select value={props.mode} options={createRunModeOptions(t)} disabled={disabled}
            ariaLabel={t("Run mode", "运行模式")} menuPreferredWidth={260} menuMinimumWidth={200}
            menuAlign="left" menuClassName="run-mode-menu" onChange={props.onModeChange} />
        </div></div>
      </div>
    </div>
  );
}
