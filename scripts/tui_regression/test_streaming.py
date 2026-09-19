"""【终端】【流式回归】检查实际屏幕和滚动历史中的内容完整性。"""
import unittest
from mock_model import MockModel
from terminal import TerminalSession


class StreamingTests(unittest.TestCase):
    """覆盖真实 HTTP、事件分发和终端渲染。"""

    def test_long_table_keeps_every_row(self):
        """生成中及结束后保留所有表格行、前置正文与思考，返回无。"""
        with MockModel() as model, TerminalSession(model.config()) as terminal:
            terminal.send(b"Output a long table\r")
            terminal.wait_for(model.ready.is_set)
            terminal.wait_for(lambda: "ROW059" in terminal.text())
            rendered = terminal.text(history=True)
            for index in range(60):
                self.assertIn(f"ROW{index:03}", rendered)
            for marker in ["ROWKEY", "THOUGHT-PREFIX", "INTRO-PREFIX"]:
                self.assertIn(marker, rendered)
            self.assertNotIn("THOUGHT-PREFIX", terminal.text())
            self.assertEqual(rendered.count("ROW000"), 1)
            model.release.set()
            terminal.wait_for(lambda: "AFTER-TABLE" in terminal.text())
            for index in range(60):
                self.assertIn(f"ROW{index:03}", terminal.text(history=True))
            self.assertNotIn(b"\x1b[3J", terminal.raw)

    def test_resize_preserves_stream_and_input(self):
        """连续改变尺寸时保留历史并完成回复，返回无。"""
        with MockModel(30) as model, TerminalSession(model.config()) as terminal:
            terminal.send(b"Output table\r")
            terminal.wait_for(model.ready.is_set)
            terminal.wait_for(lambda: "ROW029" in terminal.text())
            for width in [65, 90, 52, 80]:
                terminal.resize(width, 24)
                terminal.pump(.03)
            terminal.wait_for(lambda: "ROW029" in terminal.text())
            model.release.set()
            terminal.wait_for(lambda: "AFTER-TABLE" in terminal.text())
            for index in range(30):
                self.assertIn(f"ROW{index:03}", terminal.text(history=True))
            self.assertNotIn(b"\x1b[3J", terminal.raw)

    def test_streaming_completion_escape_preserves_draft(self):
        """生成期间关闭候选只恢复输入焦点，不打断回复或清除草稿；返回无。"""
        with MockModel(15) as model, TerminalSession(model.config()) as terminal:
            terminal.send(b"Output table\r")
            terminal.wait_for(model.ready.is_set)
            terminal.wait_for(lambda: "ROW014" in terminal.text())
            terminal.send(b"/context ")
            terminal.wait_for(lambda: "/context edit" in terminal.text())
            terminal.send(b"\x1b")
            terminal.wait_for(lambda: "/context edit" not in terminal.text())
            self.assertIn("/context", terminal.text())
            model.release.set()
            terminal.wait_for(lambda: "AFTER-TABLE" in terminal.text())
            self.assertIn("/context", terminal.screen.display[terminal.screen.cursor.y])
