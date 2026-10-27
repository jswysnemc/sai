"""【终端】【普通样式回归】检查列表续行和命令准备、执行、完成阶段的布局。"""

import json
import shlex
import unittest

from mock_activity_model import ActivityModel
from terminal import TerminalSession
from test_fullscreen import click, content_lines, lines


class DeepseekStandardStyleTests(unittest.TestCase):
    """【终端】【普通样式回归】用真实伪终端和本地模型检查展示及状态切换。"""

    def row_of(self, terminal, text):
        """参数 terminal 为会话、text 为标记；返回标记所在的可见行。"""
        return next(index for index, line in enumerate(lines(terminal)) if text in line)

    def test_nested_list_wraps_at_content_column(self):
        """无参数；中文与长标识符自动换行后，续行与嵌套列表正文对齐。"""
        with ActivityModel("read_file", {}) as model:
            model.answer = ("Introduction\n2. 跨平台沙箱能力\n  - Codex 实现：提供完整跨平台隔离实现，"
                "Linux 使用 bubblewrap，Windows 使用 RestrictedToken 与 NetworkDomainPermissions。\n  - next item")
            with TerminalSession(model.config(), columns=55, rows=30,
                    arguments=["--lang", "en-US"],
                    ready=lambda terminal: "Enter a message" in terminal.text()) as terminal:
                path = terminal.root / "fixture.txt"
                path.write_text("fixture content")
                model.arguments = {"path": str(path)}
                terminal.send(b"Inspect wrapping\r")
                terminal.wait_for(model.ready.is_set)
                model.advance.set()
                terminal.wait_for(lambda: "next item" in terminal.text())
                visible = lines(terminal)
                first = self.row_of(terminal, "Codex")
                last = self.row_of(terminal, "next item")
                self.assertGreater(last, first + 1)
                for line in visible[first + 1:last]:
                    self.assertEqual(len(line) - len(line.lstrip()), 6, repr(line))

    def test_command_transitions_keep_region_and_input_stable(self):
        """无参数；准备、执行和完成保持标题位置，输入框始终固定且不显示 JSON。"""
        with ActivityModel() as model:
            with TerminalSession(model.config(), columns=100, rows=34,
                    arguments=["--lang", "en-US"]) as terminal:
                gate = terminal.root / "release-command"
                model.command = ("printf 'RUNNING-OUT\\n'; "
                    f"while [ ! -f {shlex.quote(str(gate))} ]; do sleep 0.02; done; "
                    "printf 'FINAL-OUT\\n'")
                arguments = json.dumps({"command": model.command})
                model.argument_chunks = (arguments[:-2], arguments[-2:])
                terminal.send(b"Inspect command transitions\r")
                terminal.wait_for(model.ready.is_set)
                model.advance.set()
                terminal.wait_for(model.preparing.is_set)
                terminal.wait_for(lambda: "Preparing" in terminal.text())
                group, composer = self.row_of(terminal, "▾"), self.row_of(terminal, "→")
                self.assertNotIn('"command":', terminal.text())
                model.resume_preparation.set()
                terminal.wait_for(lambda: any(line.strip() == "└ RUNNING-OUT" for line in content_lines(terminal)))
                self.assertEqual(self.row_of(terminal, "▾"), group)
                self.assertEqual(self.row_of(terminal, "→"), composer)
                self.assertNotIn('"command":', terminal.text())
                gate.touch()
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text())
                self.assertEqual(self.row_of(terminal, "▾"), group)
                self.assertLess(self.row_of(terminal, "▾"), self.row_of(terminal, "Ran"))
                last_output = next(index for index, line in enumerate(content_lines(terminal))
                                   if line.strip() == "FINAL-OUT")
                self.assertLessEqual(self.row_of(terminal, "ANSWER-END") - last_output, 2)
                self.assertNotIn("Following latest", terminal.text())
                self.assertEqual(self.row_of(terminal, "→"), composer)
                self.assertTrue(any(line.strip() == "└ RUNNING-OUT" for line in content_lines(terminal)))
                self.assertIn("FINAL-OUT", terminal.text())
                click(terminal, 6, self.row_of(terminal, "Ran"))
                terminal.wait_for(lambda: "FINAL-OUT" not in terminal.text())
                click(terminal, 6, self.row_of(terminal, "Ran"))
                terminal.wait_for(lambda: any(line.strip() == "└ RUNNING-OUT" for line in content_lines(terminal)))
                self.assertIn("FINAL-OUT", terminal.text())
                self.assertNotIn('"command":', terminal.text())
                self.assertEqual(self.row_of(terminal, "→"), composer)


if __name__ == "__main__":
    unittest.main()
