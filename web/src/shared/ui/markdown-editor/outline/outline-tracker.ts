import { syntaxTree } from "@codemirror/language";
import { type EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { activeHeadingIndex, extractOutline, sameOutline, type OutlineHeading } from "./outline-model";

/** 大纲变化的订阅方。 */
export type OutlineListener = {
  /** 标题列表变化时调用 */
  onOutline: (headings: OutlineHeading[]) => void;
  /** 视口顶部所属标题变化时调用，-1 表示位于首个标题之前 */
  onActive: (index: number) => void;
};

/** 文档变化后重新提取大纲的延迟（毫秒），连续输入时只算最后一次。 */
const OUTLINE_DEBOUNCE_MS = 120;

/** 判定当前标题时视口顶部下移的余量（像素），标题刚滚到顶部即视为进入该节。 */
const ACTIVE_OFFSET_PX = 48;

/**
 * 构建大纲跟踪扩展。
 *
 * 文档或语法树变化时防抖重算标题，滚动时逐帧重算当前标题，
 * 结果只在真正变化时通知，避免每次按键都触发外层重渲染。
 *
 * @param listener 大纲订阅方，通过 getter 取得以便外部替换回调而不重建扩展
 * @returns CodeMirror 视图插件
 */
export function outlineTracker(listener: () => OutlineListener | null) {
  return ViewPlugin.fromClass(
    class {
      headings: OutlineHeading[] = [];
      active = -2;
      timer: ReturnType<typeof setTimeout> | undefined;
      frame = 0;

      constructor(readonly view: EditorView) {
        this.onScroll = this.onScroll.bind(this);
        view.scrollDOM.addEventListener("scroll", this.onScroll, { passive: true });
        this.schedule(0);
      }

      /**
       * 文档或语法树变化时安排重算；语法树后台解析完成时也会走到这里。
       *
       * @param update 视图更新
       * @returns 无
       */
      update(update: ViewUpdate) {
        if (update.docChanged || syntaxTree(update.state) !== syntaxTree(update.startState)) {
          this.schedule(OUTLINE_DEBOUNCE_MS);
        } else if (update.geometryChanged || update.viewportChanged) {
          this.onScroll();
        }
      }

      /**
       * 防抖重算标题列表。
       *
       * @param delay 延迟毫秒数
       * @returns 无
       */
      schedule(delay: number) {
        clearTimeout(this.timer);
        this.timer = setTimeout(() => {
          const next = extractOutline(this.view.state);
          if (!sameOutline(this.headings, next)) {
            this.headings = next;
            listener()?.onOutline(next);
          }
          this.refreshActive();
        }, delay);
      }

      /**
       * 滚动时逐帧节流地重算当前标题。
       *
       * @returns 无
       */
      onScroll() {
        if (this.frame) return;
        this.frame = requestAnimationFrame(() => {
          this.frame = 0;
          this.refreshActive();
        });
      }

      /**
       * 以视口顶部对应的文档位置判定当前标题。
       *
       * @returns 无
       */
      refreshActive() {
        const scroller = this.view.scrollDOM;
        // 1. 滚到底部时最后一节可能不足一屏，直接视为最后一个标题
        const atBottom = scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 2;
        // 2. 视口顶部换算成相对文档顶部的高度，已扣除滚动容器的内边距
        const viewportTop = scroller.getBoundingClientRect().top - this.view.documentTop;
        const position = atBottom
          ? this.view.state.doc.length
          : this.view.lineBlockAtHeight(Math.max(0, viewportTop + ACTIVE_OFFSET_PX)).from;
        const next = activeHeadingIndex(this.headings, position);
        if (next === this.active) return;
        this.active = next;
        listener()?.onActive(next);
      }

      destroy() {
        clearTimeout(this.timer);
        cancelAnimationFrame(this.frame);
        this.view.scrollDOM.removeEventListener("scroll", this.onScroll);
      }
    }
  );
}
