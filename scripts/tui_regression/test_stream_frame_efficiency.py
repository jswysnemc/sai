"""【终端】【帧效率回归】完整字符分片与等待期间的无效光标帧。"""

import re
import unittest

from mock_fragmented_model import FragmentedModel
from terminal import TerminalSession


class StreamFrameEfficiencyTests(unittest.TestCase):
    """记录真实帧输出，正文更新和动画仍按原节奏显示。"""

    def test_waiting_stream_does_not_emit_cursor_only_frames(self):
        """无参数；中文按 HTTP 字节分块仍完整显示，等待期间不重复输出纯光标帧。"""
        source = "中文片段 café 界面"
        with FragmentedModel(source, byte_chunks=True) as model, TerminalSession(
            model.config(), arguments=["--lang", "en-US", "--view", "fullscreen"], columns=100, rows=32,
        ) as terminal:
            terminal.send(b"Inspect fragmented stream")
            terminal.wait_for(lambda: "Inspect fragmented stream" in terminal.text())
            terminal.send(b"\r")
            terminal.wait_for(lambda: source in terminal.text())
            cursor = (terminal.screen.cursor.x, terminal.screen.cursor.y)
            before = len(terminal.raw)
            terminal.pump(1.2)
            output = bytes(terminal.raw[before:])
            frames = re.findall(rb"\x1b\[\?2026h(.*?)\x1b\[\?2026l", output, re.S)
            cursor_only = [frame for frame in frames if not re.sub(rb"\x1b\[(?:\?25[hl]|\d+;\d+H)", b"", frame)]
            self.assertEqual(cursor_only, [], "Unchanged screen emitted cursor-only frames")
            self.assertGreater(len(frames), 0, "Waiting animation stopped repainting")
            self.assertEqual((terminal.screen.cursor.x, terminal.screen.cursor.y), cursor)
            self.assertFalse(terminal.screen.cursor.hidden)
            self.assertNotIn("\ufffd", terminal.text())
            self.assertIn("Working", terminal.text())
            terminal.send(b"QUEUED-DRAFT")
            terminal.wait_for(lambda: "QUEUED-DRAFT" in terminal.text())
            cursor = (terminal.screen.cursor.x, terminal.screen.cursor.y)
            terminal.pump(.3)
            self.assertEqual((terminal.screen.cursor.x, terminal.screen.cursor.y), cursor)
            self.assertFalse(terminal.screen.cursor.hidden)
            model.release.set()
            terminal.wait_for(lambda: "STREAM-DONE" in terminal.text())
