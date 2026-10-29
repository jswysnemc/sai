"""【终端】【摘要回归】检查窄终端中的实际行范围、文件名与失败状态。"""

import unittest

from mock_activity_model import ActivityModel
from terminal import TerminalSession
from test_fullscreen import click, lines


class ActivitySummaryTests(unittest.TestCase):
    """通过真实工具结果和终端缩放检查折叠标题，不访问外部服务。"""

    def test_narrow_file_summary_keeps_basename_and_actual_range(self):
        """无参数；请求二十行但只返回三行时，长目录不遮挡文件名与实际范围。"""
        path = "a-long-project-directory/another-long-directory/source-file.rs"
        with ActivityModel("read_file", {"path": path, "offset": 1, "limit": 20}) as model:
            model.advance.set()
            with TerminalSession(model.config(), columns=100, rows=30,
                    arguments=["--lang", "en-US"]) as terminal:
                fixture = terminal.root / path
                fixture.parent.mkdir(parents=True)
                fixture.write_text("first\nsecond\nthird\n")
                terminal.send(b"Read the short fixture\r")
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text(), timeout=15)
                summary = next(line for line in lines(terminal) if "Read file" in line)
                self.assertIn("source-file.rs (L1–3)", summary)
                self.assertNotRegex(summary, r" · \d+(?:\.\d+)?(?:ms|s)(?: ·|$)")
                output_size = len(terminal.raw)
                terminal.resize(50, 24)
                terminal.wait_for(lambda: len(terminal.raw) > output_size + 500)
                summary = next(line for line in lines(terminal) if "Read file" in line)
                self.assertIn("source-file.rs (L1–3)", summary)
                self.assertNotIn("L1–20", summary)
                self.assertNotRegex(summary, r" · \d+(?:\.\d+)?(?:ms|s)(?: ·|$)")

    def test_narrow_command_summary_keeps_failure_visible(self):
        """无参数；长命令失败后，折叠标题优先显示失败状态而不是仅改变颜色。"""
        with ActivityModel() as model:
            model.command = "printf '%s' '" + "sample " * 30 + "'; exit 7"
            model.advance.set()
            with TerminalSession(model.config(), columns=100, rows=30,
                    arguments=["--lang", "en-US"]) as terminal:
                terminal.send(b"Execute the failing command\r")
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text(), timeout=15)
                terminal.wait_for(lambda: "Failed" in terminal.text())
                row = next(index for index, line in enumerate(lines(terminal)) if "$ " in line)
                click(terminal, 5, row)
                terminal.wait_for(lambda: any("Failed" in line and "$ " in line for line in lines(terminal)))
                output_size = len(terminal.raw)
                terminal.resize(50, 24)
                terminal.wait_for(lambda: len(terminal.raw) > output_size + 500)
                summary = next(line for line in lines(terminal) if "$ " in line)
                self.assertIn("Failed", summary)
                self.assertNotIn("Awaiting approval", summary)


if __name__ == "__main__":
    unittest.main()
