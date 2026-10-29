"""【终端】【卡片预览回归】检查真实工具的默认预览、鼠标展开和失败诊断。"""

import unittest

from mock_activity_model import ActivityModel
from terminal import TerminalSession
from test_fullscreen import click, content_lines, lines


class ToolCardPreviewTests(unittest.TestCase):
    """【终端】【卡片预览回归】使用本地模型与隔离目录，不访问外部服务。"""

    def test_search_preview_expands_and_restores_grouped_matches(self):
        """无参数；首尾预览保留文件归属，鼠标展开显示中间命中，再次点击恢复预览。"""
        with ActivityModel("grep", {"path": ".", "pattern": "TOKEN", "include": "*.txt"}) as model:
            model.advance.set()
            with TerminalSession(model.config(), columns=100, rows=40,
                    arguments=["--lang", "en-US"]) as terminal:
                for name in ["a.txt", "b.txt"]:
                    (terminal.root / name).write_text("\n".join(f"TOKEN-{index:02}" for index in range(1, 11)))
                terminal.send(b"Search local fixtures\r")
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text(), timeout=15)
                terminal.wait_for(lambda: "Searched files" in terminal.text())
                self.assertNotIn("TOKEN-05", terminal.text())
                self.assertIn("20 matches in 2 files", terminal.text())
                row = next(index for index, line in enumerate(lines(terminal)) if "Searched files" in line)
                click(terminal, 6, row)
                terminal.send(b"\x1b[1;3H")
                terminal.wait_for(lambda: "TOKEN-05" in terminal.text())
                row = next(index for index, line in enumerate(lines(terminal)) if "Searched files" in line)
                click(terminal, 6, row)
                terminal.wait_for(lambda: "TOKEN-05" not in terminal.text())
                self.assertNotIn('"stdout"', terminal.text())

    def test_failed_command_keeps_exit_code_and_stdout_error(self):
        """无参数；仅有标准输出诊断的非零退出显示真实原因，收起后仍保留退出码。"""
        with ActivityModel() as model:
            model.command = "printf 'fatal: missing dependency\\n'; exit 17"
            model.advance.set()
            with TerminalSession(model.config(), columns=100, rows=32,
                    arguments=["--lang", "en-US"]) as terminal:
                terminal.send(b"Inspect failing command\r")
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text(), timeout=15)
                self.assertIn("Failed · exit 17", terminal.text())
                self.assertTrue(any(line.strip() == "fatal: missing dependency" for line in content_lines(terminal)))
                self.assertNotIn("without stderr", terminal.text())
                row = next(index for index, line in enumerate(lines(terminal)) if "$ " in line)
                click(terminal, 6, row)
                terminal.wait_for(lambda: not any(line.strip() == "fatal: missing dependency" for line in content_lines(terminal)))
                self.assertIn("Failed · exit 17", terminal.text())

    def test_todo_default_change_record_and_expanded_plan(self):
        """无参数；新增计划默认保留变更记录，点击后显示原序完整计划，不重复任务标题。"""
        with ActivityModel("todo", {"action": "add", "texts": ["FIRST-TASK", "SECOND-TASK", "THIRD-TASK"]}) as model:
            model.advance.set()
            with TerminalSession(model.config(), columns=100, rows=40,
                    arguments=["--lang", "en-US"]) as terminal:
                terminal.send(b"Create fixture plan\r")
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text(), timeout=15)
                self.assertNotIn("Plan:", terminal.text())
                self.assertIn("2 more changes", terminal.text())
                row = next(index for index, line in enumerate(lines(terminal)) if "Added" in line)
                click(terminal, 6, row)
                terminal.wait_for(lambda: "Plan:" in terminal.text())
                visible = terminal.text()
                self.assertLess(visible.index("FIRST-TASK"), visible.index("SECOND-TASK"))
                self.assertLess(visible.index("SECOND-TASK"), visible.index("THIRD-TASK"))
                self.assertNotIn('"items"', visible)


if __name__ == "__main__":
    unittest.main()
