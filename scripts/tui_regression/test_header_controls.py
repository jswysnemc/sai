"""【终端标题】【真实回归】顶部位置指示和侧栏快速开关。"""

import unittest

from terminal import TerminalSession
from test_fullscreen import click, lines
from test_tui_view import default_config


class HeaderControlTests(unittest.TestCase):
    """真实终端验证开关命中、悬停、草稿光标与尺寸限制。"""

    def test_toggle_hover_draft_and_size_boundary(self):
        """无参数；顶部开关不发送消息，缩窄后隐藏，草稿继续编辑，返回无。"""
        config = default_config({"fullscreen": True, "sidebar": "off"})
        with TerminalSession(config, columns=160, rows=32,
                             arguments=["--lang", "en-US"]) as terminal:
            terminal.wait_for(lambda: "[ │>]" in lines(terminal)[0])
            self.assertNotIn("Ctrl+O", lines(terminal)[0])
            terminal.send(b"KEPT-DRAFT\x1b[D\x1b[D\x1b[D")
            terminal.wait_for(lambda: "KEPT-DRAFT" in terminal.text())
            column = 156
            terminal.send(f"\x1b[<35;{column + 1};1M".encode())
            terminal.wait_for(lambda: terminal.screen.buffer[0][column].reverse)
            self.assertFalse(terminal.screen.buffer[0][column].underscore)
            click(terminal, column, 0)
            terminal.wait_for(lambda: "Changes" in terminal.text() and "[<│ ]" in lines(terminal)[0])
            self.assertEqual(terminal.screen.buffer[0][column].bg, "default")
            click(terminal, column, 0)
            terminal.wait_for(lambda: "Changes" not in terminal.text() and "[ │>]" in lines(terminal)[0])
            terminal.send(b"X")
            terminal.wait_for(lambda: "KEPT-DRXAFT" in terminal.text())
            terminal.resize(80, 32)
            terminal.wait_for(lambda: "[ │>]" not in lines(terminal)[0])
            self.assertIn("KEPT-DRXAFT", terminal.text())
            terminal.resize(100, 32)
            terminal.wait_for(lambda: "[ │>]" in lines(terminal)[0])
            click(terminal, 96, 0)
            terminal.wait_for(lambda: "Changes" in terminal.text())
            self.assertIn("KEPT-DRXAFT", terminal.text())
