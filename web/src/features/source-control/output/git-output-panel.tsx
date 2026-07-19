import { ChevronDown, TerminalSquare } from "lucide-react";
import { useState } from "react";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import type { GitOutputEntry } from "../types";

/**
 * 渲染可展开的 Git 命令输出日志。
 *
 * @param props 按时间排序的操作输出
 * @returns Git 输出面板
 */
export function GitOutputPanel({ entries }: { entries: GitOutputEntry[] }) {
  const { locale, t } = useI18n();
  const [open, setOpen] = useState(false);
  const failed = entries.filter((entry) => !entry.ok).length;
  return (
    <section className={`git-output-panel${open ? " open" : ""}`}>
      <Button className="git-output-toggle" onClick={() => setOpen((value) => !value)} aria-expanded={open}>
        <TerminalSquare size={13} />
        <span>{t("Git Output", "Git 输出")}</span>
        {failed > 0 && <strong>{failed}</strong>}
        <ChevronDown size={12} />
      </Button>
      {open && (
        <div className="git-output-log">
          {entries.length === 0 && <div className="git-clean">{t("No Git commands have run yet", "尚未执行 Git 命令")}</div>}
          {entries.map((entry) => (
            <article className={entry.ok ? "success" : "failure"} key={entry.id}>
              <header>
                <code>git:{entry.action}</code>
                <time>{new Date(entry.createdAt).toLocaleTimeString(locale)}</time>
              </header>
              {entry.message && <p>{entry.message}</p>}
              {entry.stdout && <pre>{entry.stdout}</pre>}
              {entry.stderr && <pre className="stderr">{entry.stderr}</pre>}
            </article>
          ))}
        </div>
      )}
    </section>
  );
}
