"""【终端】【上下文回归】完整命令只读查看，面板承接正文并恢复输入。"""

import unittest

from terminal import TerminalSession


class ContextPanelTests(unittest.TestCase):
    """使用隔离会话检查命令补全和上下文面板。"""

    def test_completed_context_command_opens_read_only_panel(self):
        """无参数；补全后直接回车打开查看面板，不接受 edit 或 reset 参数。"""
        with TerminalSession(arguments=["--lang", "en-US"], columns=100, rows=36) as terminal:
            terminal.send(b"/cont\t")
            terminal.wait_for(lambda: "/context " in terminal.screen.display[terminal.screen.cursor.y])
            terminal.send(b"\r")
            terminal.wait_for(lambda: "Esc Close" in terminal.text(), timeout=5)
            self.assertNotIn("/context reset", terminal.text())
            self.assertNotIn("AUTO-COMPACT", terminal.text())

    def test_context_panel_replaces_composer_below_history(self):
        """无参数；内联面板紧接历史，无旧输入框残留，缩放关闭后仍能输入。"""
        with TerminalSession(arguments=["--lang", "en-US"], columns=100, rows=40) as terminal:
            before = len(terminal.raw)
            terminal.send(b"/context\r")
            terminal.wait_for(lambda: "Esc Close" in terminal.text())
            title = next(i for i, row in enumerate(terminal.screen.display) if "Esc Close" in row)
            self.assertLess(title, 12, terminal.text())
            self.assertNotIn("Shift+Tab mode", terminal.text())
            self.assertNotIn(b"\x1b[?1049h", terminal.raw[before:])
            terminal.pump(.1)
            painted = len(terminal.raw)
            terminal.pump(.6)
            self.assertEqual(len(terminal.raw), painted, "Idle panel should not repaint")
            terminal.resize(50, 24)
            terminal.wait_for(lambda: "Esc Close" in terminal.text())
            terminal.send(b"\x1b[B\x1b[6~\x1b")
            terminal.wait_for(lambda: "Shift+Tab mode" in terminal.text())
            self.assertNotIn("Esc Close", terminal.text())
            terminal.send(b"AFTER-CONTEXT")
            terminal.wait_for(lambda: "AFTER-CONTEXT" in terminal.text())
            self.assertFalse(terminal.screen.cursor.hidden)

    def test_fullscreen_context_preserves_existing_view(self):
        """无参数；全屏会话在正文下方保留面板，关闭时不切屏或丢失输入能力。"""
        with TerminalSession(arguments=["--lang", "en-US", "--view", "fullscreen"],
                             columns=100, rows=40) as terminal:
            before = len(terminal.raw)
            terminal.send(b"/context\r")
            terminal.wait_for(lambda: "Esc Close" in terminal.text())
            self.assertNotIn(b"\x1b[?1049h", terminal.raw[before:])
            self.assertNotIn(b"\x1b[?1049l", terminal.raw[before:])
            terminal.send(b"\x1b")
            terminal.wait_for(lambda: "Shift+Tab mode" in terminal.text())
            self.assertNotIn("Esc Close", terminal.text())
            terminal.send(b"AFTER-FULLSCREEN-CONTEXT")
            terminal.wait_for(lambda: "AFTER-FULLSCREEN-CONTEXT" in terminal.text())
