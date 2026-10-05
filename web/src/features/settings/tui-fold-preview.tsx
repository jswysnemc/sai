import type { DisplayConfig, FoldPreviewMode } from "../../api/contracts";
import { useI18n } from "../i18n/use-i18n";
import { ChoicePills, SettingsField, SkNumberInput } from "./kit";
import "./tui-fold-preview.css";

const SAMPLE_LINES = 12;
const MAX_LINES = 24;

type TuiFoldPreviewProps = {
  display: DisplayConfig;
  onChange: (patch: DisplayConfig) => void;
};

/**
 * 解析折叠预览模式；未知取值回退为首尾模式。
 *
 * @param value 配置文本
 * @returns 折叠预览模式
 */
function foldMode(value: unknown): FoldPreviewMode {
  return value === "head" || value === "hidden" ? value : "ends";
}

/**
 * 把折叠行数限制在可读范围内。
 *
 * @param value 原始行数
 * @param min 下限
 * @returns 夹紧后的行数
 */
function clampLines(value: unknown, min: number): number {
  const number = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(number)) return min;
  return Math.min(MAX_LINES, Math.max(min, Math.round(number)));
}

/**
 * 判断样本行在当前模式下是开头、结尾还是省略。
 *
 * @param index 从 0 起的行号
 * @param mode 折叠模式
 * @param head 保留开头行数
 * @param tail 保留结尾行数
 * @returns 行角色
 */
function lineKind(
  index: number,
  mode: FoldPreviewMode,
  head: number,
  tail: number
): "head" | "tail" | "omitted" {
  if (mode === "hidden") return "omitted";
  if (index < head) return "head";
  if (mode === "ends" && index >= SAMPLE_LINES - tail) return "tail";
  return "omitted";
}

/**
 * 渲染 TUI 折叠预览：分段选择、可点纸带与行数调节。
 *
 * @param props 当前显示配置与局部更新回调
 * @returns 折叠预览字段
 */
export function TuiFoldPreview({ display, onChange }: TuiFoldPreviewProps) {
  const { t } = useI18n();
  const mode = foldMode(display.fold_preview);
  const head = clampLines(display.fold_head_lines ?? 2, 1);
  const tail = clampLines(display.fold_tail_lines ?? 4, 0);
  const hint = mode === "head"
    ? t("Keep the first lines and hide the rest.", "只保留开头若干行，其余折叠。")
    : mode === "hidden"
      ? t("Hide the whole block until it is expanded.", "折叠时不保留正文，展开后才显示。")
      : t("Keep the first and last lines; click a bar to set that edge.", "保留开头与结尾；点击色条可设定对应边界。");

  /**
   * 按点击位置更新开头或结尾行数。
   *
   * @param index 被点击的样本行
   * @returns 无返回值
   */
  const selectLine = (index: number) => {
    if (mode === "hidden") return;
    if (mode === "head") {
      onChange({ fold_head_lines: index + 1 });
      return;
    }
    if (index < SAMPLE_LINES / 2) onChange({ fold_head_lines: index + 1 });
    else onChange({ fold_tail_lines: SAMPLE_LINES - index });
  };

  return (
    <>
      <SettingsField
        span="full"
        label={t("TUI fold preview", "TUI 折叠预览")}
        configKey="display.fold_preview"
        anchor="runtime.display.fold_preview"
        hint={hint}
      >
        <ChoicePills
          value={mode}
          options={[
            { value: "head", label: t("Keep first lines", "保留开头") },
            { value: "ends", label: t("Keep first and last", "保留首尾") },
            { value: "hidden", label: t("Hide all", "全部省略") }
          ]}
          onChange={(value) => onChange({ fold_preview: value })}
        />
        <div className="tui-fold-preview" data-mode={mode}>
          <div className="tui-fold-tape" role="group" aria-label={t("Fold preview", "折叠示意")}>
            {Array.from({ length: SAMPLE_LINES }, (_, index) => {
              const kind = lineKind(index, mode, head, tail);
              return (
                <button
                  key={index}
                  type="button"
                  className="tui-fold-line"
                  data-kind={kind}
                  disabled={mode === "hidden"}
                  aria-label={t(`Line ${index + 1}`, `第 ${index + 1} 行`)}
                  onClick={() => selectLine(index)}
                >
                  <span className="tui-fold-index">{index + 1}</span>
                  <span className="tui-fold-bar" />
                </button>
              );
            })}
          </div>
        </div>
      </SettingsField>
      {mode !== "hidden" && (
        <SettingsField
          label={t("First lines", "开头行数")}
          configKey="display.fold_head_lines"
          size="sm"
        >
          <SkNumberInput
            value={head}
            min={1}
            max={MAX_LINES}
            integer
            ariaLabel={t("First lines", "开头行数")}
            onChange={(value) => onChange({ fold_head_lines: value ?? 2 })}
          />
        </SettingsField>
      )}
      {mode === "ends" && (
        <SettingsField
          label={t("Last lines", "结尾行数")}
          configKey="display.fold_tail_lines"
          size="sm"
        >
          <SkNumberInput
            value={tail}
            min={0}
            max={MAX_LINES}
            integer
            ariaLabel={t("Last lines", "结尾行数")}
            onChange={(value) => onChange({ fold_tail_lines: value ?? 4 })}
          />
        </SettingsField>
      )}
    </>
  );
}
