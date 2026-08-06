import { useMemo } from "react";
import { diffStatusLabel } from "./diff/diff-model";
import type { DiffFile } from "./diff/diff-model";
import { parseDiff } from "./diff/diff-parser";
import { DiffIdeaView } from "./diff-idea-view";
import { DiffUnifiedView } from "./diff-unified-view";
import { ToolFileReference } from "./tool-file-reference";
import { useI18n } from "../../i18n/use-i18n";
import "./diff-view.css";

export type DiffLayout = "unified" | "side";

type DiffViewProps = {
  source: string;
  headerPath?: string;
  /** 仅渲染指定文件；工作区标签页用它避免展示整份补丁 */
  onlyPath?: string;
  /** 为 true 时隐藏文件头，避免与外层文件行重复 */
  hideHeader?: boolean;
  /** 统一单栏或左右并排 */
  layout?: DiffLayout;
};

/**
 * 以 IDE 风格渲染统一 Diff 或 Codex patch 文本。
 *
 * @param props Diff 源文本与布局
 * @returns 按文件分块、带双行号列的 Diff 视图
 */
export function DiffView({ source, headerPath, onlyPath, hideHeader = false, layout = "unified" }: DiffViewProps) {
  const { t } = useI18n();
  // 解析与字符级配对是纯计算，父组件重渲染时不应重跑
  const files = useMemo(() => selectDiffFiles(parseDiff(source), onlyPath), [onlyPath, source]);
  if (files.length === 0) return null;
  return (
    <div
      className={`structured-diff${hideHeader ? " is-compact" : ""}${layout === "side" ? " is-side" : ""}`}
      role="region"
      aria-label={t("File diff", "文件差异")}
    >
      {files.map((file, index) => (
        <DiffFileBlock
          file={file}
          hideHeader={hideHeader || (files.length === 1 && file.path === headerPath)}
          hidePath={files.length === 1 && file.path === headerPath}
          layout={layout}
          key={`${file.path}-${index}`}
        />
      ))}
    </div>
  );
}

/**
 * 从补丁中选出目标文件，兼容绝对路径和 Git 的 a/、b/ 路径前缀。
 *
 * @param files 已解析的文件差异
 * @param onlyPath 用户当前查看的文件路径
 * @returns 匹配到目标时仅返回该文件，否则返回第一项以保持可用状态
 */
export function selectDiffFiles(files: DiffFile[], onlyPath?: string): DiffFile[] {
  if (!onlyPath || files.length <= 1) return files;
  const target = normalizeDiffPath(onlyPath);
  const matched = files.find((file) => {
    const path = normalizeDiffPath(file.path);
    return path === target || target.endsWith(`/${path}`) || path.endsWith(`/${target}`);
  });
  return matched ? [matched] : files.slice(0, 1);
}

/**
 * 归一化差异路径，便于比较工作区绝对路径和 Git 相对路径。
 *
 * @param value 原始文件路径
 * @returns 去除 Git 前缀后的斜杠路径
 */
function normalizeDiffPath(value: string): string {
  return value.replaceAll("\\", "/").replace(/^(?:a|b)\//u, "").replace(/^\.\//u, "");
}

/**
 * 渲染单个文件的差异块，含文件名条与增删统计徽标。
 *
 * @param props 解析后的文件差异
 * @returns 文件差异块
 */
function DiffFileBlock({
  file,
  hideHeader,
  hidePath,
  layout
}: {
  file: DiffFile;
  hideHeader: boolean;
  hidePath: boolean;
  layout: DiffLayout;
}) {
  const { t } = useI18n();
  const status = diffStatusLabel(file.status);
  const showHead = !hideHeader;
  return (
    <section className="diff-file">
      {showHead && (
        <header className="diff-file-head">
          {!hidePath && file.path && (
            <span className="diff-file-title">
              <ToolFileReference path={file.path} label={fileName(file.path)} />
              {fileDirectory(file.path) && <span className="diff-file-directory">{fileDirectory(file.path)}</span>}
            </span>
          )}
          {!file.path && <strong>{t("Change fragment", "变更片段")}</strong>}
          <small>{t(status.en, status.zh)}</small>
          <span className="diff-file-stats">
            {file.added > 0 && <b>+{file.added}</b>}
            {file.removed > 0 && <i>-{file.removed}</i>}
          </span>
        </header>
      )}
      {file.oldPath && (
        <p className="diff-file-note">
          {t(`Renamed from ${file.oldPath}`, `由 ${file.oldPath} 重命名`)}
        </p>
      )}
      {file.status === "binary" && (
        <p className="diff-file-note">{t("Binary file not shown", "二进制文件不展示内容")}</p>
      )}
      {file.lines.length > 0 &&
        (layout === "side" ? (
          <DiffIdeaView file={file} language={languageOfPath(file.path)} />
        ) : (
          <DiffUnifiedView file={file} language={languageOfPath(file.path)} />
        ))}
    </section>
  );
}

/**
 * 从文件路径推断代码着色语言。
 *
 * @param path 文件路径
 * @returns 扩展名语言标识，无扩展名时为 undefined
 */
function languageOfPath(path: string): string | undefined {
  const name = path.split("/").pop() ?? "";
  return name.includes(".") ? name.split(".").pop() : undefined;
}

/**
 * 从差异路径中提取文件名。
 *
 * @param path 文件路径
 * @returns 文件名
 */
function fileName(path: string): string {
  return path.split(/[\\/]/u).filter(Boolean).at(-1) ?? path;
}

/**
 * 从差异路径中提取目录辅助文字。
 *
 * @param path 文件路径
 * @returns 目录路径；没有目录时返回空
 */
function fileDirectory(path: string): string {
  const normalized = path.replaceAll("\\", "/");
  const slash = normalized.lastIndexOf("/");
  return slash > -1 ? normalized.slice(0, slash) : "";
}
