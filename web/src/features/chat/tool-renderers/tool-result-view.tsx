import { EditToolView } from "./edit-tool-view";
import { GenericToolView } from "./generic-tool-view";
import { ReadToolView } from "./read-tool-view";
import { ShellToolView } from "./shell-tool-view";

type ToolResultViewProps = {
  name: string;
  argumentsText: string;
  output: string;
  headerPath?: string;
};

/**
 * 按工具类型选择专业结果渲染器。
 *
 * @param props 工具名称、参数和输出
 * @returns 工具结果视图
 */
export function ToolResultView({ name, argumentsText, output, headerPath }: ToolResultViewProps) {
  if (name === "run_command" || name.includes("background_command")) {
    return <ShellToolView argumentsText={argumentsText} output={output} />;
  }
  if (name === "read_file") {
    return <ReadToolView argumentsText={argumentsText} output={output} headerPath={headerPath} />;
  }
  if (name === "edit_file" || name === "apply_patch" || name === "write_file" || name === "replace_file_lines") {
    return <EditToolView argumentsText={argumentsText} output={output} headerPath={headerPath} />;
  }
  return <GenericToolView argumentsText={argumentsText} output={output} />;
}
