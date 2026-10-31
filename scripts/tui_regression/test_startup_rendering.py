"""【终端】【启动回归】普通终端首次绘制不泄漏图片协议。"""

import unittest
import re

from terminal import TerminalSession
from test_fullscreen import fullscreen_ready, lines
from test_tui_view import default_config


class StartupRenderingTests(unittest.TestCase):
    """在不支持图片协议的终端检查启动输出。"""

    def test_fullscreen_brand_alignment_and_unique_sidebar_toggle(self):
        """无参数；品牌三行对齐，权限颜色保留，右上角只有一个开关，不使用图片协议。"""
        with TerminalSession(default_config({"fullscreen": True}), columns=160, rows=32,
                             arguments=["--lang", "en-US"], ready=fullscreen_ready) as terminal:
            screen = lines(terminal)
            mark_rows = [(index, text) for index, text in enumerate(screen) if any(stroke in text[:100] for stroke in ["╭──╴", "╰──╮", "╶──╯"])]
            self.assertEqual(len(mark_rows), 3)
            starts = [text.index(stroke) for (_, text), stroke in zip(mark_rows, ["╭──╴", "╰──╮", "╶──╯"])]
            self.assertEqual(starts, [2, 2, 2])
            top, title = mark_rows[0]
            self.assertIn("sai", title)
            self.assertIn("YOLO", title)
            self.assertEqual(terminal.screen.buffer[top][title.index("YOLO")].fg, "ff8700")
            self.assertIn("model test", mark_rows[1][1])
            self.assertEqual(sum(line.count("\uf0db") for line in screen), 1)
            self.assertNotIn("[x]", "\n".join(line[120:] for line in screen[:3]))
            self.assertNotIn(b"\x1b_G", terminal.raw)

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
