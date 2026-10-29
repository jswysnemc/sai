"""【终端】【底栏选择回归】状态栏和快捷键提示支持拖选复制，不改变输入草稿。"""

import base64
import unittest

from mock_fragmented_model import FragmentedModel
from terminal import TerminalSession


class ComposerFooterSelectionTests(unittest.TestCase):
    """通过真实鼠标协议验证全屏底栏。"""

    def drag_copy(self, terminal, start, end, expected):
        """terminal 为终端，start/end 为列行坐标，expected 为纯文本；返回无，验证高亮和复制。"""
        mark = len(terminal.raw)
        terminal.send(f"\x1b[<0;{start[0] + 1};{start[1] + 1}M".encode())
        terminal.send(f"\x1b[<32;{end[0] + 1};{end[1] + 1}M".encode())
        first = min(start, end, key=lambda point: (point[1], point[0]))
        terminal.wait_for(lambda: terminal.screen.buffer[first[1]][first[0]].reverse, timeout=2)
        terminal.send(f"\x1b[<0;{end[0] + 1};{end[1] + 1}m".encode())
        payload = b"\x1b]52;c;" + base64.b64encode(expected.encode()) + b"\x07"
        terminal.wait_for(lambda: payload in terminal.raw[mark:], timeout=2)

    def test_status_and_key_hints_are_selectable_and_copyable(self):
        """无参数；全屏视图逐项拖选底栏文本，验证高亮、复制和后续输入。"""
        with TerminalSession(
            columns=100, rows=32, arguments=["--view", "fullscreen", "--lang", "en-US"],
        ) as terminal:
            terminal.send(b"DRAFT-UNCHANGED")
            terminal.wait_for(lambda: "DRAFT-UNCHANGED" in terminal.text())
            for columns, rows_count in ((100, 32), (60, 20)):
                terminal.resize(columns, rows_count)
                terminal.wait_for(lambda: "DRAFT-UNCHANGED" in terminal.text()
                                  and "Shift+Enter" in terminal.text())
                selected_items = ("yolo", "test", "auto", "Shift+Enter") if columns == 100 else ("yolo", "Shift+Enter")
                for selected in selected_items:
                    with self.subTest(columns=columns, selected=selected):
                        rows = terminal.text().splitlines()
                        row = next(index for index, line in enumerate(rows)
                                   if selected in line and "DRAFT-UNCHANGED" not in line
                                   and ("yolo" in line if selected != "Shift+Enter" else True))
                        column = rows[row].index(selected)
                        self.drag_copy(terminal, (column, row), (column + len(selected), row), selected)
                        self.assertIn("DRAFT-UNCHANGED", terminal.text())
            terminal.send(b"!")
            terminal.wait_for(lambda: "DRAFT-UNCHANGED!" in terminal.text())
            self.assertFalse(terminal.screen.cursor.hidden)

    def test_both_footer_rows_copy_in_both_directions_with_empty_input(self):
        """无参数；空草稿时跨行正反向拖选底栏，复制两行可见文字，单击不复制。"""
        with TerminalSession(
            columns=100, rows=32, arguments=["--view", "fullscreen", "--lang", "en-US"],
        ) as terminal:
            rows = terminal.text().splitlines()
            status = next(index for index, line in enumerate(rows) if "yolo" in line and "auto" in line)
            hints = status + 1
            start = (rows[status].index("yolo"), status)
            end = (len(rows[hints]) - 1, hints)
            expected = rows[status][start[0]:].rstrip() + "\n" + rows[hints][:-1].rstrip()
            self.drag_copy(terminal, start, end, expected)
            self.drag_copy(terminal, end, start, expected)
            mark = len(terminal.raw)
            terminal.send(f"\x1b[<0;{start[0] + 1};{start[1] + 1}M".encode())
            terminal.send(f"\x1b[<0;{start[0] + 1};{start[1] + 1}m".encode())
            terminal.pump(.2)
            self.assertNotIn(b"\x1b]52;c;", terminal.raw[mark:])
            terminal.send(b"AFTER-COPY")
            terminal.wait_for(lambda: "AFTER-COPY" in terminal.text())

    def test_streaming_footer_copy_preserves_draft_and_cursor(self):
        """无参数；模型等待期间拖选快捷键提示，不中断响应，不改写排队草稿和光标。"""
        with FragmentedModel("COPY-STREAM") as model, TerminalSession(
            model.config(), columns=100, rows=32,
            arguments=["--view", "fullscreen", "--lang", "en-US"],
        ) as terminal:
            terminal.send(b"Inspect footer")
            terminal.wait_for(lambda: "Inspect footer" in terminal.text())
            terminal.send(b"\r")
            terminal.wait_for(lambda: "COPY-STREAM" in terminal.text())
            terminal.send(b"QUEUED-DRAFT")
            terminal.wait_for(lambda: "QUEUED-DRAFT" in terminal.text())
            rows = terminal.text().splitlines()
            row = next(index for index, line in enumerate(rows) if "Ctrl+C stop" in line)
            column = rows[row].index("Ctrl+C")
            cursor = (terminal.screen.cursor.x, terminal.screen.cursor.y)
            self.drag_copy(terminal, (column, row), (column + 6, row), "Ctrl+C")
            terminal.pump(.2)
            self.assertEqual((terminal.screen.cursor.x, terminal.screen.cursor.y), cursor)
            self.assertFalse(terminal.screen.cursor.hidden)
            self.assertIn("QUEUED-DRAFT", terminal.text())
            model.release.set()
            terminal.wait_for(lambda: "STREAM-DONE" in terminal.text())
