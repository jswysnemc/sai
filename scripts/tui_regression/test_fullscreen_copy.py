"""【全屏视图】【复制回归】通过真实鼠标事件验证选区与 OSC 52 剪贴板内容。"""

import base64
import re
import unittest

from terminal import TerminalSession
from test_fullscreen import TurnModel, lines


class FullscreenCopyTests(unittest.TestCase):
    """验证拖选复制不破坏全屏会话和后续输入。"""

    def test_selection_copies_only_selected_visible_text(self):
        """发送鼠标按下、拖动和松开事件；验证复制载荷后继续对话，返回无。"""
        with TurnModel() as model, TerminalSession(model.config(), columns=100, rows=30) as terminal:
            terminal.send(b"ALPHA\r")
            terminal.wait_for(lambda: "ANSWER-ALPHA-7" in terminal.text())
            terminal.send(b"\x0f")
            terminal.wait_for(lambda: "1/1" in lines(terminal)[0])
            selected = "ANSWER-ALPHA-6"
            row = next(index for index, line in enumerate(lines(terminal)) if selected in line)
            column = lines(terminal)[row].index(selected)
            mark = len(terminal.raw)
            terminal.send(f"\x1b[<0;{column + 1};{row + 1}M".encode())
            terminal.send(f"\x1b[<32;{column + len(selected) + 1};{row + 1}M".encode())
            terminal.send(f"\x1b[<0;{column + len(selected) + 1};{row + 1}m".encode())
            pattern = rb"\x1b\]52;c;([A-Za-z0-9+/=]*)\x07"
            terminal.wait_for(lambda: re.search(pattern, bytes(terminal.raw[mark:])) is not None)
            payload = re.search(pattern, bytes(terminal.raw[mark:])).group(1)
            self.assertEqual(base64.b64decode(payload).decode(), selected)
            terminal.send(b"BRAVO\r")
            terminal.wait_for(lambda: "ANSWER-BRAVO-7" in terminal.text())
