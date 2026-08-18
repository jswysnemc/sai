import { useEffect, useRef, useState } from "react";
import { useI18n } from "../i18n/use-i18n";

/**
 * 轨迹详情里的分区内容框。
 *
 * 详情面板同屏会并列 5–8 个这样的框，每个都装着系统提示词的一段，内容普遍
 * 超出 18rem 的上限。若它们始终可滚，指针落在面板中央时滚轮几乎必然被某个
 * 内容框吃掉，外层面板反而滚不动。
 *
 * 这里把滚动权交给显式意图：未聚焦时 overflow 为 hidden，该元素不参与滚轮
 * 消费，滚轮直接冒泡给外层面板；点击或用键盘 Tab 聚焦后才切成可滚。取消也
 * 顺理成章——点击别处或按 Escape 失焦即可。
 *
 * @param title 分区标题，用作可聚焦元素的无障碍名称
 * @param body 分区正文
 * @returns 按需可滚的内容框
 */
export function DetailsBody({ title, body }: { title: string; body: string }) {
  const { t } = useI18n();
  const bodyRef = useRef<HTMLPreElement>(null);
  const [overflowing, setOverflowing] = useState(false);

  useEffect(() => {
    const node = bodyRef.current;
    if (!node) return;

    // 1. 内容是否超出上限决定要不要提示「可以点开滚动」，没超出的框不该有噪音
    const measure = () => setOverflowing(node.scrollHeight > node.clientHeight + 1);
    measure();

    // 2. 面板宽度变化会改变折行数，进而改变是否溢出，因此持续观察
    const observer = new ResizeObserver(measure);
    observer.observe(node);
    return () => observer.disconnect();
  }, [body]);

  return (
    <div
      className="trajectory-details-body-wrap"
      data-overflowing={overflowing ? "true" : undefined}
      data-hint={t("Click to scroll", "点击后可滚动")}
    >
      <pre
        ref={bodyRef}
        className="trajectory-details-body"
        tabIndex={0}
        aria-label={t(`${title}, click to scroll`, `${title}，点击后可滚动`)}
        onKeyDown={(event) => {
          // 3. Escape 交还滚动权，不必把指针移出面板再点一次
          if (event.key === "Escape") bodyRef.current?.blur();
        }}
      >
        {body}
      </pre>
    </div>
  );
}
