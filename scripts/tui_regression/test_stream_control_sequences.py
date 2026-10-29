"""【终端】【协议边界回归】未闭合控制序列不能拦截后续输入和整帧内容。"""

import unittest

from mock_fragmented_model import FragmentedModel
from terminal import TerminalSession


class StreamControlSequenceTests(unittest.TestCase):
    """在模型流式入口验证控制序列跨片段补齐后的显示。"""

    def test_open_string_control_does_not_capture_following_frames(self):
        """无参数；OSC、DCS、APC 未闭合期间输入正常，补齐后正文不缺失或重复。"""
        controls = [
            ("\x1b]8;;https://example.invalid/", "\x1b\\LINK\x1b]8;;\x1b\\"),
            ("\x1bPpayload", "\x1b\\LINK"),
            ("\x1b_Gpayload", "\x1b\\LINK"),
            ("\x1b[38;2;170;", "232;246mLINK\x1b[0m"),
        ]
        for view in ("inline", "fullscreen"):
            for opening, closing in controls:
                with self.subTest(view=view, control=opening), FragmentedModel(
                    "VISIBLE-TEXT" + opening, closing + "\nCONTROL-DONE\n",
                ) as model, TerminalSession(
                    model.config(), arguments=["--lang", "en-US", "--view", view], columns=100, rows=32,
                    kitty_graphics=opening.startswith("\x1b_"),
                ) as terminal:
                    terminal.send(b"Inspect control")
                    terminal.wait_for(lambda: "Inspect control" in terminal.text())
                    response_start = len(terminal.raw)
                    terminal.send(b"\r")
                    terminal.wait_for(model.ready.is_set)
                    terminal.wait_for(lambda: "VISIBLE-TEXT" in terminal.text())
                    self.assertNotIn(opening.encode(), terminal.raw[response_start:])
                    before = len(terminal.raw)
                    terminal.send(b"AFTER-CONTROL")
                    terminal.wait_for(lambda: "AFTER-CONTROL" in terminal.text(), timeout=2)
                    self.assertNotIn(opening.encode(), terminal.raw[before:])
                    self.assertFalse(terminal.screen.cursor.hidden)
                    model.release.set()
                    terminal.wait_for(lambda: "CONTROL-DONE" in terminal.text())
                    self.assertIn("LINK", terminal.text())
                    self.assertEqual(terminal.text().count("VISIBLE-TEXT"), 1)
                    self.assertIn((opening + closing).encode(), terminal.raw[response_start:])
                    # 1. 【终端】【模拟器边界】pyte 不识别 DCS，完整协议以原始输出断言验证
                    if not opening.startswith("\x1bP"):
                        self.assertNotIn("payload", terminal.text())
