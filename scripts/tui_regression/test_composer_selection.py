"""【终端】【输入框拖选】验证真实鼠标选区、复制及草稿保留。"""

import base64
import unittest

from terminal import TerminalSession


class ComposerSelectionTests(unittest.TestCase):
    """【终端】【输入框交互】使用隔离终端，不提交输入或访问模型。"""

    def test_drag_selects_and_copies_input_without_changing_draft(self):
        """无参数；拖选产生高亮与 OSC 52，草稿保留，返回无。"""
        with TerminalSession(columns=80, rows=24, arguments=["--view", "fullscreen", "--lang", "en-US"],
                             ready=lambda t: "Enter a message" in t.text()) as terminal:
            terminal.pump(.4)
            terminal.send(b"INPUT-SELECT-FIXTURE")
            terminal.wait_for(lambda: "INPUT-SELECT-FIXTURE" in terminal.text())
            rows = terminal.text().splitlines()
            row = next(i for i, line in enumerate(rows) if "INPUT-SELECT-FIXTURE" in line)
            column = rows[row].index("SELECT")
            terminal.send(f"\x1b[<0;{column + 1};{row + 1}M".encode())
            terminal.send(f"\x1b[<32;{column + 7};{row + 1}M".encode())
            terminal.wait_for(lambda: terminal.screen.buffer[row][column].reverse)
            terminal.send(f"\x1b[<0;{column + 7};{row + 1}m".encode())
            expected = b"\x1b]52;c;" + base64.b64encode(b"SELECT") + b"\x07"
            terminal.wait_for(lambda: expected in terminal.raw)
            self.assertIn("INPUT-SELECT-FIXTURE", terminal.text())
            terminal.send(b"!")
            terminal.wait_for(lambda: "INPUT-SELECT-FIXTURE!" in terminal.text())
            self.assertFalse(terminal.screen.buffer[row][column].reverse)


if __name__ == "__main__":
    unittest.main()
