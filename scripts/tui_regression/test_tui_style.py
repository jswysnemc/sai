"""【终端】【样式回归】验证两种展示样式、持久化、全屏切换及窗口缩放。"""

import json
import unittest

from terminal import TerminalSession
from test_fullscreen import TurnModel


class TuiStyleTests(unittest.TestCase):
    """使用隔离配置与真实伪终端验证展示样式，不访问外部模型。"""

    def stored_config(self, terminal):
        """参数 terminal 为隔离终端；返回磁盘中的完整配置。"""
        return json.loads((terminal.root / "config/sai/config.jsonc").read_text())

    def test_default_classic_and_launch_override(self):
        """无参数；验证旧配置保留背景，启动覆盖不修改磁盘偏好。"""
        with TerminalSession(arguments=["--lang", "en-US"], columns=100, rows=30) as terminal:
            self.assertIn("→", terminal.text())
            self.assertIn(b"\x1b[48;5;235m", terminal.raw)
        with TerminalSession(arguments=["--lang", "en-US", "--style", "deepseek"], columns=100, rows=30) as terminal:
            self.assertIn("DeepSeek style", terminal.text())
            self.assertIn("→", terminal.text())
            self.assertNotIn("Execution details", terminal.text())
            self.assertEqual(self.stored_config(terminal)["display"]["tui_style"], "classic")

    def test_style_toggle_persists_and_preserves_history(self):
        """无参数；验证双向切换、命令补全及已提交工具输出保留。"""
        with TerminalSession(arguments=["--lang", "en-US"], columns=100, rows=30) as terminal:
            terminal.send(b"!echo STYLE-HISTORY\r")
            terminal.wait_for(lambda: "STYLE-HISTORY" in terminal.text())
            terminal.pump(.4)
            terminal.send(b"/sty\t")
            terminal.wait_for(lambda: "/style" in terminal.screen.display[terminal.screen.cursor.y])
            terminal.send(b"\r")
            terminal.wait_for(lambda: "terminal style: deepseek" in terminal.text())
            self.assertIn("→", terminal.text())
            self.assertIn("STYLE-HISTORY", terminal.text(history=True))
            self.assertEqual(self.stored_config(terminal)["display"]["tui_style"], "deepseek")
            before = len(terminal.raw)
            terminal.send(b"/style\r")
            terminal.wait_for(lambda: "terminal style: classic" in terminal.text())
            self.assertIn("→", terminal.text())
            self.assertNotIn("Execution details", terminal.text())
            self.assertIn(b"\x1b[?1049l", terminal.raw[before:])
            self.assertIn("STYLE-HISTORY", terminal.text(history=True))
            self.assertEqual(self.stored_config(terminal)["display"]["tui_style"], "classic")

    def test_style_switch_keeps_fullscreen_and_conversation(self):
        """无参数；验证全屏内切换不退出备用屏，仍能继续同一会话。"""
        with TurnModel() as model:
            config = model.config()
            config["display"].update(fullscreen=True, tui_style="deepseek")
            with TerminalSession(config, arguments=["--lang", "en-US"], columns=100, rows=30) as terminal:
                terminal.send(b"ALPHA\r")
                terminal.wait_for(lambda: "ANSWER-ALPHA-7" in terminal.text(), timeout=15)
                terminal.pump(.4)
                terminal.wait_for(lambda: "→" in terminal.text())
                before = len(terminal.raw)
                terminal.send(b"/style classic\r")
                terminal.wait_for(lambda: "terminal style: classic" in terminal.text())
                self.assertNotIn(b"\x1b[?1049l", terminal.raw[before:])
                self.assertTrue(self.stored_config(terminal)["display"]["fullscreen"])
                terminal.send(b"BRAVO\r")
                terminal.wait_for(lambda: "ANSWER-BRAVO-7" in terminal.text(), timeout=15)
                terminal.pump(.4)
                terminal.wait_for(lambda: "→" in terminal.text())
                terminal.send(b"/style deepseek\r")
                terminal.wait_for(lambda: "terminal style: deepseek" in terminal.text())
                self.assertIn("ANSWER-BRAVO-7", terminal.text())

    def test_invalid_style_does_not_change_preference(self):
        """无参数；非法样式展示用法，不修改偏好或发起模型请求。"""
        with TerminalSession(arguments=["--lang", "en-US", "--style", "deepseek"], columns=100, rows=30) as terminal:
            terminal.send(b"/style invalid\r")
            terminal.wait_for(lambda: "usage: /style [classic|deepseek]" in terminal.text())
            self.assertIn("→", terminal.text())
            self.assertEqual(self.stored_config(terminal)["display"]["tui_style"], "classic")

    def test_configuration_form_offers_both_styles(self):
        """无参数；验证统一配置表单能选择两种样式，取消时不写入偏好。"""
        with TerminalSession(arguments=["--lang", "en-US"], columns=100, rows=32) as terminal:
            terminal.send(b"/config\r")
            terminal.wait_for(lambda: "SAI CONFIG" in terminal.text())
            terminal.send(b"6\r")
            terminal.wait_for(lambda: "ADVANCED" in terminal.text())
            terminal.send(b"3\r")
            terminal.wait_for(lambda: "GLOBAL PARAMETERS" in terminal.text())
            terminal.send(b"5\r")
            terminal.wait_for(lambda: "DISPLAY" in terminal.text())
            terminal.send(b"\x1b[B" * 12 + b"\r")
            terminal.wait_for(lambda: "classic" in terminal.text() and "deepseek" in terminal.text())
            terminal.send(b"\x1b[B\r")
            terminal.wait_for(lambda: "DISPLAY" in terminal.text() and "deepseek" in terminal.text())
            terminal.send(b"\x1b")
            terminal.wait_for(lambda: "GLOBAL PARAMETERS" in terminal.text())
            self.assertEqual(self.stored_config(terminal)["display"]["tui_style"], "classic")

    def test_deepseek_narrow_resize_keeps_input_cursor(self):
        """无参数；窄窗口中的多行宽字符输入在缩放后仍能编辑。"""
        with TerminalSession(arguments=["--lang", "en-US", "--style", "deepseek"], columns=50, rows=16,
                             ready=lambda terminal: "→" in terminal.text()) as terminal:
            terminal.send(("中文输入 " * 8 + " END-STYLE").encode())
            terminal.wait_for(lambda: "END-STYLE" in terminal.visible_text())
            expected = "END-STYLE"
            for columns, rows, suffix in [(30, 12, "Z"), (80, 24, "X"), (40, 16, "Y")]:
                terminal.resize(columns, rows)
                terminal.wait_for(lambda: expected in terminal.visible_text())
                self.assertLess(terminal.screen.cursor.x, columns)
                self.assertLess(terminal.screen.cursor.y, rows)
                terminal.send(suffix.encode())
                expected += suffix
                terminal.wait_for(lambda: expected in terminal.visible_text())
