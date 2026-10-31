"""【终端】【视图回归】验证全屏与内联视图、旧配置迁移、持久化、会话内切换及窗口缩放。"""

import json
import unittest

from terminal import TerminalSession
from test_fullscreen import TurnModel, fullscreen_ready

ENTER_ALT = b"\x1b[?1049h"
LEAVE_ALT = b"\x1b[?1049l"


def default_config(display):
    """参数 display 为显示配置；返回不访问网络的最小隔离配置。"""
    return {"active_provider": "test", "providers": [{
        "id": "test", "display_name": "Test", "base_url": "http://127.0.0.1:1/v1",
        "api_key": "local-fixture", "models": ["test"], "default_model": "test"}], "display": display}


class TuiViewTests(unittest.TestCase):
    """使用隔离配置与真实伪终端验证视图选择，不访问外部模型。"""

    def stored_config(self, terminal):
        """参数 terminal 为隔离终端；返回磁盘中的完整配置。"""
        return json.loads((terminal.root / "config/sai/config.jsonc").read_text())

    def test_launch_override_does_not_change_preference(self):
        """无参数；内联偏好保留主屏，启动参数与旧别名进入全屏但不修改磁盘偏好。"""
        with TerminalSession(arguments=["--lang", "en-US"], columns=100, rows=30) as terminal:
            self.assertIn("→", terminal.text())
            self.assertNotIn(ENTER_ALT, terminal.raw)
        for arguments in [["--view", "fullscreen"], ["--style", "deepseek"]]:
            with TerminalSession(arguments=["--lang", "en-US"] + arguments, columns=100, rows=30,
                                 ready=fullscreen_ready) as terminal:
                self.assertIn(ENTER_ALT, terminal.raw)
                self.assertNotIn("DeepSeek", terminal.text())
                self.assertFalse(self.stored_config(terminal)["display"]["fullscreen"])

    def test_legacy_style_migrates_to_view(self):
        """无参数；旧 deepseek 样式迁移为全屏，旧 classic 样式保留内联。"""
        legacy = default_config({"tui_style": "deepseek", "fullscreen": False})
        with TerminalSession(legacy, arguments=["--lang", "en-US"], columns=100, rows=30,
                             ready=fullscreen_ready) as terminal:
            self.assertIn(ENTER_ALT, terminal.raw)
        classic = default_config({"tui_style": "classic", "fullscreen": False})
        with TerminalSession(classic, arguments=["--lang", "en-US"], columns=100, rows=30) as terminal:
            self.assertNotIn(ENTER_ALT, terminal.raw)

    def test_view_toggle_persists_and_preserves_history(self):
        """无参数；验证双向切换、命令补全及已提交工具输出保留，旧样式键不再写入。"""
        with TerminalSession(arguments=["--lang", "en-US"], columns=100, rows=30) as terminal:
            terminal.send(b"!echo VIEW-HISTORY\r")
            terminal.wait_for(lambda: "VIEW-HISTORY" in terminal.text())
            terminal.pump(.4)
            terminal.send(b"/vie\t")
            terminal.wait_for(lambda: "/view" in terminal.screen.display[terminal.screen.cursor.y])
            terminal.send(b"\r")
            terminal.wait_for(lambda: "terminal view: fullscreen" in terminal.text())
            self.assertIn(ENTER_ALT, terminal.raw)
            self.assertIn("VIEW-HISTORY", terminal.text(history=True))
            stored = self.stored_config(terminal)["display"]
            self.assertTrue(stored["fullscreen"])
            self.assertNotIn("tui_style", stored)
            before = len(terminal.raw)
            terminal.send(b"/view\r")
            terminal.wait_for(lambda: LEAVE_ALT in terminal.raw[before:])
            terminal.wait_for(lambda: "terminal view: inline" in terminal.text())
            self.assertIn("→", terminal.text())
            self.assertIn("VIEW-HISTORY", terminal.text(history=True))
            self.assertFalse(self.stored_config(terminal)["display"]["fullscreen"])

    def test_view_switch_keeps_conversation(self):
        """无参数；全屏与内联往返切换后仍能继续同一会话。"""
        with TurnModel() as model:
            config = model.config()
            config["display"]["fullscreen"] = True
            with TerminalSession(config, arguments=["--lang", "en-US"], columns=100, rows=30) as terminal:
                terminal.send(b"ALPHA\r")
                terminal.wait_for(lambda: "ANSWER-ALPHA-7" in terminal.text(), timeout=15)
                terminal.pump(.4)
                before = len(terminal.raw)
                terminal.send(b"/view inline\r")
                terminal.wait_for(lambda: "terminal view: inline" in terminal.text())
                self.assertIn(LEAVE_ALT, terminal.raw[before:])
                self.assertFalse(self.stored_config(terminal)["display"]["fullscreen"])
                terminal.send(b"BRAVO\r")
                terminal.wait_for(lambda: "ANSWER-BRAVO-7" in terminal.text(), timeout=15)
                terminal.pump(.4)
                before = len(terminal.raw)
                terminal.send(b"/view fullscreen\r")
                terminal.wait_for(lambda: "terminal view: fullscreen" in terminal.text())
                self.assertIn(ENTER_ALT, terminal.raw[before:])
                self.assertIn("ANSWER-BRAVO-7", terminal.text())

    def test_invalid_view_does_not_change_preference(self):
        """无参数；非法视图展示用法，不修改偏好或发起模型请求。"""
        with TerminalSession(arguments=["--lang", "en-US", "--view", "fullscreen"], columns=100, rows=30,
                             ready=fullscreen_ready) as terminal:
            terminal.send(b"/view invalid\r")
            terminal.wait_for(lambda: "usage: /view [inline|fullscreen]" in terminal.text())
            self.assertIn("→", terminal.text())
            self.assertFalse(self.stored_config(terminal)["display"]["fullscreen"])

    def test_configuration_form_toggles_fullscreen(self):
        """无参数；显示偏好表单可切换全屏启动，取消时不写入偏好。"""
        with TerminalSession(arguments=["--lang", "en-US"], columns=100, rows=32) as terminal:
            terminal.send(b"/config\r")
            terminal.wait_for(lambda: "SAI CONFIG" in terminal.text())
            terminal.send(b"6\r")
            terminal.wait_for(lambda: "ADVANCED" in terminal.text())
            terminal.send(b"3\r")
            terminal.wait_for(lambda: "GLOBAL PARAMETERS" in terminal.text())
            terminal.send(b"5\r")
            terminal.wait_for(lambda: "DISPLAY" in terminal.text() and "Start fullscreen" in terminal.text())
            self.assertNotIn("Terminal style", terminal.text())
            terminal.send(b"\x1b[B" * 6 + b"\r")
            terminal.wait_for(lambda: any("Start fullscreen" in line and "● on" in line
                                          for line in terminal.text().splitlines()))
            terminal.send(b"\x1b")
            terminal.wait_for(lambda: "GLOBAL PARAMETERS" in terminal.text())
            self.assertFalse(self.stored_config(terminal)["display"]["fullscreen"])

    def test_fullscreen_narrow_resize_keeps_input_cursor(self):
        """无参数；窄窗口中的多行宽字符输入在缩放后仍能编辑。"""
        with TerminalSession(arguments=["--lang", "en-US", "--view", "fullscreen"], columns=50, rows=16,
                             ready=lambda terminal: "→" in terminal.text()) as terminal:
            terminal.send(("中文输入 " * 8 + " END-VIEW").encode())
            terminal.wait_for(lambda: "END-VIEW" in terminal.visible_text())
            expected = "END-VIEW"
            for columns, rows, suffix in [(30, 12, "Z"), (80, 24, "X"), (40, 16, "Y")]:
                terminal.resize(columns, rows)
                terminal.wait_for(lambda: expected in terminal.visible_text())
                self.assertLess(terminal.screen.cursor.x, columns)
                self.assertLess(terminal.screen.cursor.y, rows)
                terminal.send(suffix.encode())
                expected += suffix
                terminal.wait_for(lambda: expected in terminal.visible_text())


if __name__ == "__main__":
    unittest.main()
