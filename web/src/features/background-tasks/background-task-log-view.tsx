import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import type { BackgroundTask, BackgroundTaskOutput } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { ArrowDown, ArrowUp, ChevronLeft, Pause, Play } from "../../shared/ui/icons";
import { SegmentedControl } from "../../shared/ui/segmented-control";
import { useI18n } from "../i18n/use-i18n";
import { backgroundTaskLogLines, backgroundTaskLogTruncated, type BackgroundTaskStream } from "./background-task-log-model";
import "./background-task-logs.css";

type TaskLogViewProps = {
  task: BackgroundTask;
  output?: BackgroundTaskOutput;
  loading: boolean;
  error: Error | null;
  stream: BackgroundTaskStream;
  onStreamChange: (stream: BackgroundTaskStream) => void;
  onBack: () => void;
};

/**
 * 在同一面板显示带行号的日志视口，支持通道切换、翻页与可暂停的尾部跟随。
 * @param props 当前任务、输出状态、所选通道和导航回调
 * @returns 紧凑日志查看器
 */
export function BackgroundTaskLogView({ task, output, loading, error, stream, onStreamChange, onBack }: TaskLogViewProps) {
  const { t } = useI18n();
  const [follow, setFollow] = useState(true);
  const viewport = useRef<HTMLDivElement>(null);
  const lines = useMemo(() => backgroundTaskLogLines(output, stream), [output, stream]);
  const truncated = backgroundTaskLogTruncated(output, stream);

  // 1. 【后台任务】【日志跟随】仅在跟随开启时定位尾部，暂停期间保留用户滚动位置
  useEffect(() => {
    if (follow && viewport.current) viewport.current.scrollTop = viewport.current.scrollHeight;
  }, [lines, follow]);
  useEffect(() => { setFollow(true); }, [stream]);
  useEffect(() => { viewport.current?.focus(); }, []);

  /**
   * 将视口移到首尾或翻页并暂停自动跟随。
   * @param target 目标位置或相对页数
   * @returns 无返回值
   */
  const scroll = (target: "start" | "end" | number) => {
    const node = viewport.current;
    if (!node) return;
    setFollow(false);
    node.scrollTop = target === "start" ? 0 : target === "end" ? node.scrollHeight : node.scrollTop + target * node.clientHeight;
  };

  /**
   * 处理日志视口快捷键，Tab 继续用于标准焦点导航。
   * @param event 面板键盘事件
   * @returns 无返回值
   */
  const handleKey = (event: KeyboardEvent<HTMLElement>) => {
    if (event.ctrlKey || event.altKey || event.metaKey) return;
    if (event.key === "Escape") { event.preventDefault(); onBack(); return; }
    if (event.target !== viewport.current) return;
    if (event.key === "q" || event.key === "Enter") onBack();
    else if (event.key === "f") setFollow((current) => !current);
    else if (event.key === "g" || event.key === "Home") scroll("start");
    else if (event.key === "G" || event.key === "End") scroll("end");
    else if (event.key === "PageUp" || event.key === "b") scroll(-1);
    else if (event.key === "PageDown" || event.key === " ") scroll(1);
    else if (["ArrowUp", "ArrowDown", "j", "k"].includes(event.key)) {
      setFollow(false);
      viewport.current!.scrollTop += ["ArrowUp", "k"].includes(event.key) ? -24 : 24;
    } else return;
    event.preventDefault();
  };

  return <section className="background-task-log-view" aria-label={t("Task logs", "任务日志")} onKeyDown={handleKey}>
    <header className="background-task-log-heading">
      <Button variant="ghost" size="small" onClick={onBack}><ChevronLeft size={14} />{t("Tasks", "任务列表")}</Button>
      <strong title={task.label}>{task.label}</strong>
      <Button variant="ghost" size="small" aria-pressed={follow} onClick={() => setFollow((current) => !current)}>
        {follow ? <Pause size={12} /> : <Play size={12} />}{follow ? t("Following", "跟随开启") : t("Paused", "跟随暂停")}
      </Button>
    </header>
    <div className="background-task-log-toolbar">
      <SegmentedControl value={stream} onChange={onStreamChange} ariaLabel={t("Output stream", "输出通道")}
        options={[{ value: "stdout", label: "STDOUT" }, { value: "stderr", label: "STDERR" }, { value: "all", label: t("All", "全部") }]} />
      <div><Button variant="ghost" size="icon" onClick={() => scroll("start")} aria-label={t("First line", "首行")} title={t("First line", "首行")}><ArrowUp size={14} /></Button>
        <Button variant="ghost" size="icon" onClick={() => scroll("end")} aria-label={t("Last line", "末行")} title={t("Last line", "末行")}><ArrowDown size={14} /></Button></div>
    </div>
    <p className="background-task-log-hint">{t(`${lines.length} buffered lines`, `缓冲区 ${lines.length} 行`)}{stream === "all" && t(" · grouped by stream, not time", " · 按通道分组，非时间顺序")}
      {truncated && t(` · last ${output?.tail_lines} lines per stream at most; earlier output omitted`, ` · 每个通道最多末尾 ${output?.tail_lines} 行，更早输出已截断`)}</p>
    {error && <p className="background-task-inline-error" role="alert">{t("Could not read output: ", "读取输出失败：")}{error.message}</p>}
    <div className="background-task-log-viewport" ref={viewport} tabIndex={0} role="region" aria-label={t("Task output", "任务输出")} aria-busy={loading}
      onScroll={() => { const node = viewport.current; if (node && node.scrollHeight - node.scrollTop - node.clientHeight > 4) setFollow(false); }}>
      {loading ? <p role="status">{t("Loading output…", "正在读取输出…")}</p> : lines.length ? lines.map((line) => (
        <div className={`background-task-log-line ${line.stream}`} key={`${line.stream}-${line.number}`}>
          <span className="background-task-line-number" aria-hidden>{line.number}</span>
          <code>{stream === "all" && <span className="background-task-stream-tag">[{line.stream}] </span>}{line.text || " "}</code>
        </div>
      )) : !error && <p>{t("No output in this stream", "当前通道暂无输出")}</p>}
    </div>
    <footer className="background-task-log-footer hidden sm:flex">{t("↑/↓ Scroll · PgUp/PgDn Page · g/G First/last · f Follow · Esc Back", "↑/↓ 滚动 · PgUp/PgDn 翻页 · g/G 首尾 · f 跟随 · Esc 返回")}</footer>
  </section>;
}
