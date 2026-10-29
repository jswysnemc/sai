"""【终端】【位置稳定回归】连续展开历史详情时，标题不能在页面上下往返跳动。"""

import unittest

from mock_activity_model import ActivityModel
from terminal import TerminalSession
from test_fullscreen import click, lines


class ActivityPositionStabilityTests(unittest.TestCase):
    """【终端】【历史位置】在真实伪终端中记录标题与输入框的绝对行坐标。"""

    def row_of(self, terminal, text):
        """参数 terminal 为终端、text 为定位文本；返回首个匹配行坐标。"""
        return next(index for index, line in enumerate(lines(terminal)) if text in line)

    def test_history_expansion_keeps_group_and_composer_positions(self):
        """无参数；历史详情连续展开、收起时标题和输入框保持原位，返回无。"""
        for rows in [24, 40]:
            with self.subTest(rows=rows):
                self.check_history_expansion(rows)

    def check_history_expansion(self, rows):
        """参数 rows 为终端行数；验证三次展开、收起循环的坐标，返回无。"""
        with ActivityModel("read_file", {"path": "fixture.txt"}) as model:
            model.advance.set()
            with TerminalSession(model.config(), columns=100, rows=rows,
                                 ready=lambda t: "Enter a message" in t.text()) as terminal:
                (terminal.root / "fixture.txt").write_text("\n".join(f"LINE-{i:02}" for i in range(60)))
                terminal.send(b"Read history fixture\r")
                terminal.wait_for(lambda: "ANSWER-END" in terminal.text())
                terminal.send(b"\x1b[1;5H")
                terminal.wait_for(lambda: "1 read" in terminal.text() and "Read file" in terminal.text())
                click(terminal, 6, self.row_of(terminal, "Read file"))
                terminal.wait_for(lambda: "LINE-" not in terminal.text())
                group = self.row_of(terminal, "1 read")
                tool = self.row_of(terminal, "Read file")
                composer = self.row_of(terminal, "→")
                for _ in range(3):
                    click(terminal, 6, self.row_of(terminal, "Read file"))
                    terminal.wait_for(lambda: "LINE-" in terminal.text())
                    self.assertEqual(self.row_of(terminal, "1 read"), group, terminal.text())
                    self.assertEqual(self.row_of(terminal, "Read file"), tool, terminal.text())
                    self.assertEqual(self.row_of(terminal, "→"), composer)
                    click(terminal, 6, self.row_of(terminal, "Read file"))
                    terminal.wait_for(lambda: "LINE-" not in terminal.text())
                    self.assertEqual(self.row_of(terminal, "1 read"), group, terminal.text())
                    self.assertEqual(self.row_of(terminal, "Read file"), tool, terminal.text())
                    self.assertEqual(self.row_of(terminal, "→"), composer)


if __name__ == "__main__":
    unittest.main()
