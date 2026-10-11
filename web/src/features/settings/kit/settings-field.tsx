import { useId, type ReactNode } from "react";
import { fieldAnchorId } from "../search/field-anchor";
import { cx } from "./class-names";
import { FieldContext } from "./field-context";
import { SettingsRow, type SettingsRowLayout } from "./settings-row";
import "./settings-field.css";

/** 控件最大宽度档位：xs 6rem、sm 10rem、md 16rem、lg 24rem、full 占满单元格。 */
export type ControlSize = "xs" | "sm" | "md" | "lg" | "full";

type SettingsFieldProps = {
  /** 字段标签 */
  label: ReactNode;
  /** 说明文字，自动换行 */
  hint?: ReactNode;
  /** 错误文字；存在时替换说明并把控件标为无效 */
  error?: ReactNode;
  /** 对应的配置键，只出现在标签提示与搜索索引中 */
  configKey?: string;
  /** 搜索定位锚点 */
  anchor?: string;
  /** 控件最大宽度档位 */
  size?: ControlSize;
  /** 为 full 时跨整行，并改为上下排布 */
  span?: "full";
  /** 强制行排布；缺省时按控件宽度选择 */
  layout?: SettingsRowLayout;
  /** 紧跟标签的徽标，例如「仅本浏览器」 */
  badge?: ReactNode;
  /** 标签行右侧的辅助操作 */
  aside?: ReactNode;
  className?: string;
  children: ReactNode;
};

/**
 * 根据字段宽度选择行排布。
 *
 * 1. 调用方显式传入 layout 时直接使用
 * 2. 整行控件改为上下排布
 * 3. 其余字段宽屏左文右控，窄屏上下叠
 *
 * @param layout 显式排布
 * @param size 控件宽度档位
 * @param span 是否跨整行
 * @returns 行排布
 */
function resolveLayout(
  layout: SettingsRowLayout | undefined,
  size: ControlSize,
  span?: "full"
): SettingsRowLayout {
  if (layout) return layout;
  if (span === "full" || size === "full") return "stack";
  return "responsive";
}

/**
 * 渲染设置字段：标签、控件与说明组成一行。
 *
 * 控件通过字段上下文自动关联标签与说明，调用方无需手写 id。
 *
 * @param props 标签、说明、错误、配置键、锚点、宽度档位与控件
 * @returns 设置字段
 */
export function SettingsField({
  label,
  hint,
  error,
  configKey,
  anchor,
  size = "md",
  span,
  layout,
  badge,
  aside,
  className,
  children
}: SettingsFieldProps) {
  const baseId = useId();
  const controlId = `${baseId}-control`;
  const labelId = `${baseId}-label`;
  const message = error ?? hint;
  const hintId = message ? `${baseId}-hint` : undefined;
  const title = badge ? <>{label}{badge}</> : label;
  const description = error ? <span className="is-error">{error}</span> : hint;
  return (
    <SettingsRow
      id={anchor ? fieldAnchorId(anchor) : undefined}
      className={cx("sk-field", span === "full" && "is-full", className)}
      title={title}
      titleId={labelId}
      description={description}
      descriptionId={hintId}
      aside={aside}
      layout={resolveLayout(layout, size, span)}
      htmlFor={controlId}
      configKey={configKey}
      size={size}
    >
      <FieldContext.Provider
        value={{
          controlId,
          labelId,
          hintId,
          invalid: Boolean(error),
          labelText: typeof label === "string" ? label : undefined
        }}
      >
        {children}
      </FieldContext.Provider>
    </SettingsRow>
  );
}

type FieldGridProps = {
  /** 最多列数；默认两列，短字段较多时用三列 */
  columns?: 1 | 2 | 3;
  className?: string;
  children: ReactNode;
};

/**
 * 把字段栅格子项展开为分组卡片里的独立行。
 *
 * 列数属性保留给调用方，视觉上统一为单列细分隔行。
 *
 * @param props 最多列数与字段节点
 * @returns 字段栅格
 */
export function FieldGrid({ columns = 2, className, children }: FieldGridProps) {
  return <div className={cx("sk-grid", className)} data-columns={columns}>{children}</div>;
}
