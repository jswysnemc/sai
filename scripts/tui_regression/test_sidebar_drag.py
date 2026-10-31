"""【终端侧栏】【拖动回归】检查按住鼠标期间的边界绘制和分界线悬停范围。"""

import os
import shlex
import shutil
import threading
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
            self.assert_divider_hover_is_isolated(terminal)

    def test_divider_hover_waits_for_initial_git_snapshot(self):
        """【终端侧栏】【异步取样回归】无参数，返回无；初始 Git 查询延迟时仍只比较悬停变化。"""
        real_git = shutil.which("git")
        self.assertIsNotNone(real_git)

        def prepare(root, environment, binary):
            """【终端侧栏】【查询门控】参数为隔离目录、环境和程序；仅阻塞后台仓库发现，返回无。"""
            commands = root / "commands"
            commands.mkdir()
            wrapper = commands / "git"
            gate = shlex.quote(str(root / "allow-git"))
            wrapper.write_text(f'''#!/bin/sh
case "$*" in
  *"rev-parse --show-toplevel"*)
    while [ ! -f {gate} ]; do sleep 0.005; done
    ;;
esac
exec {shlex.quote(real_git)} "$@"
''')
            wrapper.chmod(0o755)
            environment["PATH"] = str(commands) + os.pathsep + environment["PATH"]

        with TerminalSession(default_config({"fullscreen": True}), columns=160, rows=32,
                             arguments=["--lang", "en-US"], prepare=prepare) as terminal:
            self.assertIn("Checking working tree...", terminal.text())
            original_send = terminal.send

            def release_query():
                """【终端侧栏】【查询放行】无参数，返回无；允许受控后台查询交付初始快照。"""
                (terminal.root / "allow-git").write_text("go")

            def send(data):
                """【终端侧栏】【竞争重放】data 为输入字节；旧取样逻辑在悬停时同时交付 Git 结果，返回无。"""
                if b"[<35;" in data:
                    release_query()
                original_send(data)

            terminal.send = send
            release = threading.Timer(.5, release_query)
            release.start()
            try:
                self.assert_divider_hover_is_isolated(terminal)
            finally:
                release.cancel()
                release.join()

    def assert_divider_hover_is_isolated(self, terminal):
        """【终端侧栏】【悬停取样】terminal 为隔离终端；等待 Git 首帧后验证仅分界线变化，返回无。"""
        terminal.wait_for(lambda: "Not a Git repository" in terminal.text())
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
