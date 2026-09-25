"""【终端】【交互回归】检查面板返回后仍能输入且光标可见。"""
import unittest
import time
from mock_model import MockModel
from terminal import TerminalSession


def compaction_panel_open(terminal):
    """terminal 为当前会话；压缩策略面板打开时为真。"""
    text = terminal.text()
    return any(marker in text for marker in ("Usage ratio", "占用比例", "AUTO-COMPACT", "自动压缩"))


class InteractionTests(unittest.TestCase):
    """覆盖面板生命周期及输入框恢复。"""

    def assert_large_paste_remains_responsive(self, terminal):
        """terminal 为隔离终端；断言十万字符折叠后立即响应后续文字，返回无。"""
        text = "中abc\n" * 20_000
        terminal.send(b"\x1b[200~" + text.encode() + b"\x1b[201~")
        terminal.wait_for(lambda: "99999 chars]" in terminal.text())
        started = time.monotonic()
        terminal.send(b"FOLLOWUP")
        terminal.wait_for(lambda: "FOLLOWUP" in terminal.text(), timeout=.5)
        self.assertLess(time.monotonic() - started, .5)
        self.assertIn("99999 chars]", terminal.text())

    def test_large_paste_keeps_idle_input_responsive(self):
        """空闲输入框显示粘贴长度后及时接受后续文字；返回无。"""
        with TerminalSession() as terminal:
            self.assert_large_paste_remains_responsive(terminal)

    def test_large_paste_keeps_streaming_input_responsive(self):
        """生成期间粘贴长文本后及时接受后续文字，且回复可继续；返回无。"""
        with MockModel(15) as model, TerminalSession(model.config()) as terminal:
            terminal.send(b"Output a table\r")
            terminal.wait_for(model.ready.is_set)
            self.assert_large_paste_remains_responsive(terminal)
            model.release.set()
            terminal.wait_for(lambda: "AFTER-TABLE" in terminal.text())
            self.assertIn("FOLLOWUP", terminal.text())

    def test_command_arguments_complete_before_execution(self):
        """Tab 连续补全参数，首次回车接受候选，再次回车执行；返回无。"""
        with TerminalSession() as terminal:
            terminal.send(b"/cont\t")
            terminal.wait_for(lambda: "/context " in terminal.screen.display[terminal.screen.cursor.y])
            terminal.send(b"\r")
            terminal.wait_for(lambda: "/context edit" in terminal.screen.display[terminal.screen.cursor.y])
            self.assertNotIn("Usage ratio", terminal.text())
            self.assertNotIn("占用比例", terminal.text())
            terminal.send(b"\r")
            terminal.wait_for(lambda: compaction_panel_open(terminal))
            terminal.send(b"q")
            terminal.wait_for(lambda: "cancelled" in terminal.text() or "已取消" in terminal.text())

    def test_file_completion_ignores_old_query(self):
        """快速更新文件查询后只显示最新候选，关闭面板不改草稿；返回无。"""
        with TerminalSession() as terminal:
            (terminal.root / "alpha-file.txt").write_text("")
            (terminal.root / "beta-file.txt").write_text("")
            terminal.send(b"@alpha")
            terminal.send(b"\x7f\x7f\x7f\x7f\x7fbeta")
            terminal.wait_for(lambda: "beta-file.txt" in terminal.text())
            self.assertNotIn("alpha-file.txt", terminal.text())
            terminal.send(b"\x1b")
            terminal.wait_for(lambda: "beta-file.txt" not in terminal.text())
            self.assertIn("@beta", terminal.text())
            terminal.send(b"\x1b[B")
            terminal.pump(.15)
            self.assertNotIn("beta-file.txt", terminal.text())
            self.assertIn("@beta", terminal.text())

    def test_candidate_page_keys_keep_input_focus(self):
        """候选分页不打开全文浏览，Esc 只关闭候选；返回无。"""
        with TerminalSession() as terminal:
            terminal.send(b"/context ")
            terminal.wait_for(lambda: "/context reset" in terminal.text())
            terminal.send(b"\x1b[6~\r")
            terminal.wait_for(lambda: "/context reset" in terminal.screen.display[terminal.screen.cursor.y])
            self.assertNotIn("Full transcript", terminal.text())
            terminal.send(b"\x1b")
            terminal.pump(.15)
            self.assertIn("/context reset", terminal.screen.display[terminal.screen.cursor.y])

    def test_overlay_restores_input_and_cursor(self):
        """打开并取消配置面板后可执行命令，返回无。"""
        with TerminalSession() as terminal:
            terminal.send(b"/context edit\r")
            terminal.wait_for(lambda: compaction_panel_open(terminal))
            terminal.send(b"q")
            terminal.wait_for(lambda: "cancelled" in terminal.text() or "已取消" in terminal.text())
            terminal.send(b"!echo INPUT-RESTORED\r")
            terminal.wait_for(lambda: "INPUT-RESTORED" in terminal.text())
            self.assertFalse(terminal.screen.cursor.hidden)
            self.assertLess(terminal.screen.cursor.y, terminal.screen.lines)
