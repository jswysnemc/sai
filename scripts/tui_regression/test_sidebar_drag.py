"""【终端侧栏】【拖动回归】检查按住鼠标期间的边界绘制和分界线悬停范围。"""

import unittest

from terminal import TerminalSession
from test_fullscreen import lines
from test_sidebar_controls import divider_column
from test_tui_view import default_config


class SidebarDragTests(unittest.TestCase):
    """在真实终端中检查逐次拖动帧，不只断言松开鼠标后的最终尺寸。"""

    def test_drag_updates_background_before_mouse_release(self):
        """无参数；拖动每帧同步输入框背景、全高分界线与鼠标位置，返回无。"""
        with TerminalSession(default_config({"fullscreen": True}), columns=160, rows=32,
                             arguments=["--lang", "en-US"]) as terminal:
            terminal.wait_for(lambda: "Changes" in terminal.text())
            terminal.send(b"KEPT-DRAFT")
            terminal.wait_for(lambda: "KEPT-DRAFT" in terminal.text())
            start = divider_column(terminal)
            terminal.send(f"\x1b[<0;{start + 1};6M".encode())
            for target in [96, 110, 80, 125, 90]:
                terminal.send(f"\x1b[<32;{target + 1};6M".encode())
                terminal.wait_for(lambda: divider_column(terminal) == target, timeout=3)
                row = next(index for index, line in enumerate(lines(terminal))
                           if "KEPT-DRAFT" in line[:target])
                left = terminal.screen.buffer[row][5].bg
                self.assertNotEqual(left, "default")
                self.assertEqual(terminal.screen.buffer[row][target - 1].bg, left)
                self.assertTrue(all(terminal.screen.buffer[row][column].bg == "default"
                                    for column in range(target + 1, 160)),
                                f"拖动到 {target} 列时，输入框旧背景必须立即清除")
                self.assertEqual(lines(terminal)[row][target], "│")
                self.assertEqual(lines(terminal)[31][target], "│")
            terminal.send(b"\x1b[<0;91;6m")
            terminal.wait_for(lambda: divider_column(terminal) == 90)

    def test_divider_hover_only_changes_divider_cells(self):
        """无参数；悬停只修改分界线单元格，不显示辅助提示或改变两侧文字，返回无。"""
        with TerminalSession(default_config({"fullscreen": True}), columns=160, rows=32,
                             arguments=["--lang", "en-US"]) as terminal:
            terminal.wait_for(lambda: "Changes" in terminal.text())
            terminal.pump(.2)
            divider = divider_column(terminal)
            before = [[terminal.screen.buffer[row][col]
                       for col in range(160)] for row in range(32)]
            terminal.send(f"\x1b[<35;{divider + 1};6M".encode())
            terminal.wait_for(lambda: terminal.screen.buffer[5][divider] != before[5][divider])
            after = [[terminal.screen.buffer[row][col]
                      for col in range(160)] for row in range(32)]
            changed = [(row, col) for row in range(32) for col in range(160)
                       if before[row][col] != after[row][col]]
            self.assertTrue(changed)
            self.assertTrue(all(col == divider for _, col in changed),
                            f"悬停不能修改分界线之外的单元格: {changed[:12]}")
            self.assertNotIn("Drag divider to resize", terminal.text())

    def test_many_drag_events_follow_latest_pointer_and_preserve_typing(self):
        """无参数；密集拖动收敛到最新位置，松开后的普通按键仍按原顺序处理，返回无。"""
        with TerminalSession(default_config({"fullscreen": True}), columns=160, rows=32,
                             arguments=["--lang", "en-US"]) as terminal:
            terminal.wait_for(lambda: "Changes" in terminal.text())
            start = divider_column(terminal)
            output_before = len(terminal.raw)
            terminal.send(f"\x1b[<0;{start + 1};6M".encode())
            events = b"".join(f"\x1b[<32;{column + 1};6M".encode()
                              for column in list(range(70, 130)) * 4 + [94])
            terminal.send(events + b"\x1b[<0;95;6mAFTER-DRAG")
            terminal.wait_for(lambda: divider_column(terminal) == 94
                              and "AFTER-DRAG" in terminal.text(), timeout=3)
            written = len(terminal.raw) - output_before
            self.assertLess(written, 100_000, f"不能逐帧重放积压的拖动事件，实际输出 {written} 字节")


if __name__ == "__main__":
    unittest.main()
