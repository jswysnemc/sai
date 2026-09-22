import { ArrowLeft, ArrowRight } from "lucide-react";
import { Children, useEffect, useRef, type ReactNode } from "react";
import { useI18n } from "../../i18n/use-i18n";
import { Button } from "../../../shared/ui/button/button";
import { useStackedPaneState } from "./stacked-pane-state";
import "./source-control-split-view.css";
import "./stacked-pane.css";

type SourceControlSplitViewProps = {
  className: string;
  /** 详情区标题，展示在返回栏上 */
  detailTitle?: string;
  /**
   * 详情侧的选中标识。有值时进入详情，清空时回到列表。
   */
  detailKey?: string | null;
  /** 保留给调用方，列表始终是入口，不再因此跳过列表。 */
  preferDetail?: boolean;
  /** 返回按钮与进入详情的名称。 */
  listLabel?: string;
  detailLabel?: string;
  children: ReactNode;
};

/**
 * 渲染 Git 的单列路由：先列表，选中后进入详情，返回再回到列表。
 *
 * @param props 外层类名、详情标题与列表、详情内容
 * @returns 一次只显示列表或详情的 Git 视图
 */
export function SourceControlSplitView(props: SourceControlSplitViewProps) {
  const { t } = useI18n();
  const detailRef = useRef<HTMLDivElement>(null);
  const { pane, direction, showList, showDetail } = useStackedPaneState();
  const [list, detail] = Children.toArray(props.children);

  useEffect(() => {
    if (props.detailKey) showDetail();
    else showList();
  }, [props.detailKey, showDetail, showList]);

  useEffect(() => {
    if (pane !== "detail") return;
    detailRef.current?.scrollTo({ top: 0 });
  }, [pane, props.detailKey]);

  return (
    <div
      className={`source-control-stacked ${direction} ${props.className}`}
      data-layout="route"
      data-pane={pane}
    >
      <div className="source-control-stacked-pane" hidden={pane !== "list"}>
        {props.detailLabel && (
          <div className="source-control-stacked-back">
            <Button variant="ghost" size="small" onClick={showDetail}>
              {props.detailLabel}<ArrowRight size={14} />
            </Button>
          </div>
        )}
        {list}
      </div>
      <div className="source-control-stacked-pane" hidden={pane !== "detail"} ref={detailRef}>
        <div className="source-control-stacked-back">
          <Button variant="ghost" size="small" onClick={showList}>
            <ArrowLeft size={14} />{props.listLabel ?? t("Back", "返回")}
          </Button>
          {props.detailTitle && <span title={props.detailTitle}>{props.detailTitle}</span>}
        </div>
        {detail}
      </div>
    </div>
  );
}
