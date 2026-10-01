"""【终端】【键盘协议回归】退出后主屏与备用屏都不残留 sai 压入的键盘增强层。"""
import os
import re
import signal
import termios
import time
import unittest
from terminal import TerminalSession


def enhancement_stacks(raw):
    """按顺序回放切屏与压栈、出栈序列，返回 (主屏层数, 备用屏层数)。"""
    stacks = {False: 0, True: 0}
    alternate = False
    for match in re.finditer(rb"\x1b\[\?1049([hl])|\x1b\[>(\d*)u|\x1b\[<(\d*)u", raw):
        screen, push, pop = match.groups()
        if screen is not None:
            alternate = screen == b"h"
        elif push is not None:
            stacks[alternate] += 1
        else:
            stacks[alternate] = max(0, stacks[alternate] - int(pop or b"1"))
    return stacks[False], stacks[True]


class KeyboardProtocolTests(unittest.TestCase):
    """覆盖全屏切换与退出时键盘增强协议的配对。"""

    def exit_and_collect(self, terminal):
        """双击 Ctrl+C 退出并等待恢复提示；返回完整输出字节。"""
        terminal.send(b"\x03")
        time.sleep(.1)
        terminal.send(b"\x03")
        terminal.wait_for(lambda: b"sai resume" in terminal.raw, timeout=15)
        return bytes(terminal.raw)

    def test_inline_exit_keeps_stacks_balanced(self):
        """普通模式双击 Ctrl+C 退出，不残留键盘增强层且回到 cooked 模式；返回无。"""
        with TerminalSession(kitty_keyboard=True) as terminal:
            raw = self.exit_and_collect(terminal)
            self.assertEqual(enhancement_stacks(raw), (0, 0))
            terminal.pump(.3)
            lflag = termios.tcgetattr(terminal.fd)[3]
            self.assertTrue(lflag & termios.ICANON and lflag & termios.ECHO)

    def test_fullscreen_round_trip_keeps_stacks_balanced(self):
        """进出全屏后退出，两块屏幕的键盘增强栈都回到零；返回无。"""
        with TerminalSession(kitty_keyboard=True) as terminal:
            self.assertIn(b"\x1b[>", terminal.raw, "harness should enable the protocol")
            terminal.send(b"\x0f")
            terminal.wait_for(lambda: terminal.raw.count(b"\x1b[?1049h") >= 1)
            terminal.send(b"\x0f")
            terminal.wait_for(lambda: terminal.raw.count(b"\x1b[?1049l") >= 1)
            raw = self.exit_and_collect(terminal)
            self.assertEqual(enhancement_stacks(raw), (0, 0))

    def test_exit_from_fullscreen_keeps_stacks_balanced(self):
        """停在全屏视图里直接退出，同样不残留任何层；返回无。"""
        with TerminalSession(kitty_keyboard=True) as terminal:
            terminal.send(b"\x0f")
            terminal.wait_for(lambda: b"\x1b[?1049h" in terminal.raw)
            raw = self.exit_and_collect(terminal)
            self.assertEqual(enhancement_stacks(raw), (0, 0))
            self.assertTrue(raw.rfind(b"\x1b[?1049l") > raw.rfind(b"\x1b[?1049h"))


    def assert_signal_restores_terminal(self, terminal, signum):
        """发送 signum 并等待退出；断言终端回到 cooked 模式、协议与备用屏都已退出；返回无。"""
        before = len(terminal.raw)
        os.kill(terminal.pid, signum)
        deadline = time.monotonic() + 5
        status = None
        while time.monotonic() < deadline:
            terminal.pump(.05)
            pid, status = os.waitpid(terminal.pid, os.WNOHANG)
            if pid:
                break
        self.assertIsNotNone(status, "sai should exit on the signal")
        self.assertEqual(os.waitstatus_to_exitcode(status), 128 + signum)
        terminal.pump(.2)
        tail = bytes(terminal.raw[before:])
        self.assertIn(b"\x1b[?1000l", tail, "mouse capture should be disabled")
        self.assertIn(b"\x1b[?2004l", tail, "bracketed paste should be disabled")
        self.assertEqual(enhancement_stacks(bytes(terminal.raw)), (0, 0))
        lflag = termios.tcgetattr(terminal.fd)[3]
        self.assertTrue(lflag & termios.ICANON and lflag & termios.ECHO, "terminal should leave raw mode")

    def test_sigterm_in_inline_mode_restores_terminal(self):
        """普通模式收到 SIGTERM：恢复 cooked 模式并退出；返回无。"""
        with TerminalSession(kitty_keyboard=True) as terminal:
            self.assert_signal_restores_terminal(terminal, signal.SIGTERM)

    def test_signals_in_fullscreen_restore_terminal(self):
        """全屏模式收到 SIGTERM / SIGHUP：退出备用屏并恢复终端；返回无。"""
        for signum in (signal.SIGTERM, signal.SIGHUP):
            with self.subTest(signal=signum), TerminalSession(kitty_keyboard=True) as terminal:
                terminal.send(b"\x0f")
                terminal.wait_for(lambda: b"\x1b[?1049h" in terminal.raw)
                self.assert_signal_restores_terminal(terminal, signum)
                raw = bytes(terminal.raw)
                self.assertTrue(raw.rfind(b"\x1b[?1049l") > raw.rfind(b"\x1b[?1049h"))


    def test_closing_the_terminal_exits_promptly(self):
        """关闭终端窗口（主端关闭、内核发 SIGHUP）后 sai 在 3 秒内退出，不在后台空转；返回无。"""
        for fullscreen in (False, True):
            with self.subTest(fullscreen=fullscreen), TerminalSession(kitty_keyboard=True) as terminal:
                if fullscreen:
                    terminal.send(b"\x0f")
                    terminal.wait_for(lambda: b"\x1b[?1049h" in terminal.raw)
                os.close(terminal.fd)
                deadline = time.monotonic() + 3
                exited = False
                while time.monotonic() < deadline:
                    pid, _ = os.waitpid(terminal.pid, os.WNOHANG)
                    if pid:
                        exited = True
                        break
                    time.sleep(.05)
                if not exited:
                    os.kill(terminal.pid, signal.SIGKILL)
                    os.waitpid(terminal.pid, 0)
                self.assertTrue(exited, "sai kept running after the terminal closed")


if __name__ == "__main__":
    unittest.main()
