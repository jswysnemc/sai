"""【终端】【旧步骤回归】验证末尾十项、较早步骤入口和默认展开的详情滚动。"""

import json
import unittest

from mock_activity_model import ActivityModel
from terminal import TerminalSession
from test_fullscreen import click, lines


class DeepseekOlderStepsTests(unittest.TestCase):
    """【终端】【旧步骤回归】通过真实工具组检查历史折叠交互。"""

    def row_of(self, terminal, text):
        """参数 terminal 为会话、text 为标记；返回标记所在的可见行。"""
        return next(index for index, line in enumerate(lines(terminal)) if text in line)

    def test_group_initially_shows_tail_and_reveals_older_steps_on_click(self):
        """无参数；末尾十项保留标题，近期详情展开，独立入口仍能恢复和收起更早步骤。"""
        with ActivityModel("read_file") as model:
            model.advance.set()
            model.tool_calls = [{"index": index, "id": f"read-{index}", "type": "function",
                "function": {"name": "read_file", "arguments": json.dumps({"path": f"step-{index:02}.txt"})}}
                for index in range(12)]
            with TerminalSession(model.config(), columns=100, rows=40,
                    arguments=["--lang", "en-US"]) as terminal:
                for index in range(12):
                    (terminal.root / f"step-{index:02}.txt").write_text("\n".join(
                        f"CONTENT-{index:02}-{line:02}" for line in range(40)))
                terminal.send(b"Inspect the tool group\r")
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text(), timeout=20)
                terminal.send(b"\x1b[1;3F")
                terminal.wait_for(lambda: "12 reads · 1 thought" in terminal.text())
                self.assertIn("12 reads · 1 thought", terminal.text())
                self.assertIn("CONTENT-11", terminal.text())
                self.assertIn("Following latest", terminal.text())
                pinned = self.row_of(terminal, "Show 3 earlier steps")
                self.assertEqual(pinned, self.row_of(terminal, "12 reads · 1 thought") + 1)
                for key in [b"\x1b[5;3~", b"\x1b[1;3H", b"\x1b[6;3~", b"\x1b[1;3F"]:
                    terminal.send(key)
                    terminal.pump(.2)
                    self.assertEqual(self.row_of(terminal, "Show 3 earlier steps"), pinned)
                    self.assertEqual(terminal.text().count("Show 3 earlier steps"), 1)
                terminal.send(b"\x1b[1;3H")
                terminal.wait_for(lambda: "Show 3 earlier steps" in terminal.text())
                visible = lines(terminal)
                group = self.row_of(terminal, "12 reads · 1 thought")
                earlier = self.row_of(terminal, "Show 3 earlier steps")
                tool = self.row_of(terminal, "step-02.txt")
                columns = [len(visible[row]) - len(visible[row].lstrip()) for row in [group, earlier, tool]]
                self.assertEqual(columns[1], columns[0] + 2)
                self.assertEqual(columns[2], columns[1])
                for index in range(2):
                    self.assertNotIn(f"step-{index:02}.txt", terminal.text())
                self.assertIn("Click to follow latest", terminal.text())
                composer = self.row_of(terminal, "→")
                terminal.send(b"\x1b[1;3F")
                terminal.wait_for(lambda: "CONTENT-11-39" in terminal.text())
                click(terminal, 6, self.row_of(terminal, "Show 3 earlier steps"))
                terminal.wait_for(lambda: "step-00.txt" in terminal.text())
                self.assertIn("Hide 3 earlier steps", terminal.text())
                self.assertNotIn("Following latest", terminal.text())
                self.assertEqual(self.row_of(terminal, "→"), composer)
                terminal.send(b"\x1b[1;3F")
                terminal.wait_for(lambda: "CONTENT-11-39" in terminal.text())
                self.assertEqual(self.row_of(terminal, "Hide 3 earlier steps"), pinned)
                click(terminal, 6, self.row_of(terminal, "Hide 3 earlier steps"))
                terminal.wait_for(lambda: "step-00.txt" not in terminal.text())
                self.assertIn("step-02.txt", terminal.text())
                self.assertIn("Show 3 earlier steps", terminal.text())
                terminal.send(b"\x1b[1;3F")
                terminal.wait_for(lambda: "step-11.txt" in terminal.text())
                self.assertEqual(self.row_of(terminal, "→"), composer)


if __name__ == "__main__":
    unittest.main()
