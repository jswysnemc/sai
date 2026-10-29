"""【终端】【启动回归】普通终端首次绘制不泄漏图片协议。"""

import unittest
import re

from terminal import TerminalSession


class StartupRenderingTests(unittest.TestCase):
    """在不支持图片协议的终端检查启动输出。"""

    def test_plain_terminal_does_not_receive_kitty_graphics(self):
        """无参数；欢迎页和首次输入不发送 Kitty 图片控制序列，避免显示协议正文。"""
        with TerminalSession(arguments=["--lang", "en-US"]) as terminal:
            self.assertNotIn(b"\x1b_G", terminal.raw)
            self.assertNotIn("Ga=d", terminal.text())
            terminal.send(b"FIRST-INPUT")
            terminal.wait_for(lambda: "FIRST-INPUT" in terminal.text())

    def test_empty_history_does_not_repaint_welcome(self):
        """无参数；首次启动的空历史不清屏重绘欢迎页，保留一次正文绘制和输入框绘制。"""
        with TerminalSession(arguments=["--lang", "en-US"]) as terminal:
            welcome = re.findall(rb"sai\x1b\[0m \x1b\[2m\d+\.\d+", terminal.raw)
            self.assertEqual(len(welcome), 1)
