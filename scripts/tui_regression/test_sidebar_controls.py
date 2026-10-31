"""【终端侧栏】【真实交互】验证命令开关、关闭控件和固定分界线拖动。"""

import unittest

from mock_activity_model import ActivityModel
from terminal import TerminalSession
from test_fullscreen import TurnModel, click, lines
from test_git_preview import prepare_repository
from test_tui_view import default_config


def divider_column(terminal):
    """参数 terminal 为终端；返回顶部固定分界线列号。"""
    return lines(terminal)[0].index("│")


def drag_divider(terminal, target, row=5):
    """参数 terminal 为终端、target 为目标列、row 为拖动行；拖动当前分界线，返回无。"""
    start = divider_column(terminal)
    terminal.send(f"\x1b[<0;{start + 1};{row + 1}M".encode())
    terminal.send(f"\x1b[<32;{target + 1};{row + 1}M".encode())
    terminal.send(f"\x1b[<0;{target + 1};{row + 1}m".encode())
    terminal.wait_for(lambda: divider_column(terminal) == target)


class SidebarControlTests(unittest.TestCase):
    """使用实际终端输入验证尺寸及可见性状态，不直接修改运行时内部字段。"""

    def test_default_off_commands_and_auto_breakpoint(self):
        """无参数；配置关闭、手动开启和自动断点使用同一布局，返回无。"""
        config = default_config({"fullscreen": True, "sidebar": "off"})
        with TerminalSession(config, columns=160, rows=32,
                             arguments=["--lang", "en-US"]) as terminal:
            self.assertNotIn("Changes", terminal.text())
            terminal.send(b"/tui sidebar on\r")
            terminal.wait_for(lambda: "Changes" in terminal.text())
            self.assertEqual(divider_column(terminal), 120)
            terminal.resize(100, 32)
            terminal.wait_for(lambda: "Changes" in terminal.text()
                              and divider_column(terminal) == 64)
            terminal.send(b"/tui sidebar auto\r")
            terminal.wait_for(lambda: "Changes" not in terminal.text())
            terminal.resize(160, 32)
            terminal.wait_for(lambda: "Changes" in terminal.text())
            terminal.send(b"/tui sidebar off\r")
            terminal.wait_for(lambda: "Changes" not in terminal.text())
            terminal.resize(200, 40)
            terminal.pump(.2)
            self.assertNotIn("Changes", terminal.text())
            terminal.send(b"/tui sidebar on\r")
            terminal.wait_for(lambda: "Changes" in terminal.text())

    def test_drag_close_hover_and_draft_cursor_retention(self):
        """无参数；拖动、关闭和终端缩放保留草稿光标，关闭按钮使用反色，返回无。"""
        config = default_config({"fullscreen": True})
        with TerminalSession(config, columns=160, rows=32,
                             arguments=["--lang", "en-US"]) as terminal:
            terminal.wait_for(lambda: "Changes" in terminal.text())
            terminal.send(b"KEPT-DRAFT\x1b[D\x1b[D\x1b[D")
            terminal.wait_for(lambda: "KEPT-DRAFT" in terminal.text())
            terminal.pump(.1)
            cursor = terminal.screen.cursor.x
            drag_divider(terminal, 96)
            self.assertEqual(terminal.screen.cursor.x, cursor)
            self.assertIn("KEPT-DRAFT", terminal.text())
            terminal.resize(180, 32)
            terminal.wait_for(lambda: divider_column(terminal) == 116)
            self.assertEqual(terminal.screen.cursor.x, cursor)
            # 1. 【终端侧栏】【控件悬停】关闭按钮反色，不给标题增加下划线
            terminal.send(b"\x1b[<35;178;2M")
            terminal.wait_for(lambda: terminal.screen.buffer[1][177].reverse)
            self.assertFalse(terminal.screen.buffer[1][177].underscore)
            click(terminal, 177, 1)
            terminal.wait_for(lambda: "Changes" not in terminal.text())
            self.assertIn("KEPT-DRAFT", terminal.text())
            self.assertEqual(terminal.screen.cursor.x, cursor)
            terminal.resize(200, 32)
            terminal.pump(.2)
            self.assertNotIn("Changes", terminal.text())
            self.assertIn("KEPT-DRAFT", terminal.text())

    def test_review_drag_and_return_restore_separate_sidebar_width(self):
        """无参数；审查区和普通侧栏分别保留宽度，拖动底部分界线不移动输入光标，返回无。"""
        with TurnModel() as model:
            config = model.config()
            config["display"]["fullscreen"] = True
            with TerminalSession(config, columns=160, rows=32, prepare=prepare_repository,
                                 arguments=["--lang", "en-US"]) as terminal:
                terminal.wait_for(lambda: "tracked.txt" in terminal.text())
                terminal.send(b"KEPT-DRAFT\x1b[D\x1b[D")
                terminal.wait_for(lambda: "KEPT-DRAFT" in terminal.text())
                terminal.pump(.1)
                cursor = terminal.screen.cursor.x
                drag_divider(terminal, 96)
                row = next(index for index, line in enumerate(lines(terminal))
                           if "tracked.txt" in line[96:])
                click(terminal, 156, row)
                terminal.wait_for(lambda: "working-line" in terminal.text())
                self.assertEqual(divider_column(terminal), 80)
                drag_divider(terminal, 70, row=31)
                self.assertEqual(terminal.screen.cursor.x, cursor)
                self.assertIn("Review", lines(terminal)[1][70:])
                self.assertIn("KEPT-DRAFT", terminal.text())
                terminal.resize(100, 32)
                terminal.wait_for(lambda: divider_column(terminal) == 40)
                self.assertFalse(any("CODE-ROW" in line[:40] for line in lines(terminal)))
                self.assertIn("KEPT-DRAFT", terminal.text())
                terminal.resize(160, 32)
                terminal.wait_for(lambda: divider_column(terminal) == 70)
                self.assertEqual(terminal.screen.cursor.x, cursor)
                terminal.send(b"\x1b[<35;11;6M")
                terminal.wait_for(lambda: not terminal.screen.buffer[31][70].reverse)
                click(terminal, 157, 1)
                terminal.wait_for(lambda: "Review" not in lines(terminal)[1])
                self.assertEqual(divider_column(terminal), 96)
                self.assertIn("KEPT-DRAFT", terminal.text())
                self.assertEqual(terminal.screen.cursor.x, cursor)

    def test_sidebar_commands_execute_during_streaming(self):
        """无参数；生成期间开关立即生效，不进入消息队列，也不丢失当前草稿，返回无。"""
        with ActivityModel() as model:
            with TerminalSession(model.config(), columns=160, rows=32,
                                 arguments=["--lang", "en-US"]) as terminal:
                terminal.send(b"Inspect controls\r")
                terminal.wait_for(model.ready.is_set, timeout=10)
                terminal.send(b"/tui sidebar off\r")
                terminal.wait_for(lambda: "Changes" not in terminal.text())
                terminal.send(b"/tui sidebar on\r")
                terminal.wait_for(lambda: "Changes" in terminal.text())
                terminal.send(b"STREAM-DRAFT")
                terminal.wait_for(lambda: "STREAM-DRAFT" in terminal.text())
                drag_divider(terminal, 100)
                click(terminal, 157, 1)
                terminal.wait_for(lambda: "Changes" not in terminal.text())
                self.assertIn("STREAM-DRAFT", terminal.text())
                self.assertNotIn("Queued messages", terminal.text())


if __name__ == "__main__":
    unittest.main()
