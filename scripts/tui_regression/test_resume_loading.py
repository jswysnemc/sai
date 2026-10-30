"""【会话恢复】【加载回归】验证其他工作区的慢索引不会阻塞当前列表。"""

import os
import unittest
from contextlib import ExitStack, contextmanager

from terminal import TerminalSession
from test_resume import Workspaces


@contextmanager
def blocked_foreign_index(fixture):
    """fixture 为双工作区数据；暂时阻塞外部索引读取，退出时恢复普通文件，返回无。"""
    index = fixture.scope(fixture.b) / "index.json"
    contents = index.read_bytes()
    index.unlink()
    os.mkfifo(index)
    # 1. 【会话恢复】【慢读取】保持写端打开，读取必须等待 EOF
    writer = os.open(index, os.O_RDWR | os.O_NONBLOCK)
    os.write(writer, contents)
    try:
        yield
    finally:
        # 2. 【会话恢复】【解除阻塞】先恢复文件，再关闭写端，兼容尚未开始读取的任务
        try:
            if index.parent.is_dir():
                restored = index.with_suffix(".ready")
                restored.write_bytes(contents)
                os.replace(restored, index)
        finally:
            os.close(writer)


@unittest.skipUnless(hasattr(os, "mkfifo"), "requires POSIX named pipes")
class ResumeLoadingTests(unittest.TestCase):
    """通过命名管道确定性模拟未完成的外部索引读取。"""

    def test_current_picker_does_not_wait_for_foreign_index(self):
        """无参数；阻塞外部索引，验证当前首屏与取消操作无需等待，返回无。"""
        fixture = Workspaces()
        with TerminalSession(prepare=fixture.setup, workspace="workspace-a") as terminal:
            with blocked_foreign_index(fixture):
                terminal.send(b"/resume\r")
                terminal.wait_for(lambda: "ALPHA-SESSION" in terminal.text(), timeout=2)
                self.assertNotIn("BETA-SESSION", terminal.text())
                terminal.send(b"\x1b")
                terminal.wait_for(lambda: "cancelled" in terminal.text() or "已取消会话" in terminal.text(), timeout=2)

    def test_loading_allows_scope_switch_and_cancel(self):
        """无参数；全量读取阻塞时仍可搜索、切回当前列表并取消，返回无。"""
        fixture = Workspaces()
        with TerminalSession(prepare=fixture.setup, workspace="workspace-a") as terminal:
            before = fixture.current(fixture.a)
            with blocked_foreign_index(fixture):
                terminal.send(b"/resume\r")
                terminal.wait_for(lambda: "ALPHA-SESSION" in terminal.text(), timeout=2)
                terminal.send(b"\tBETA")
                terminal.wait_for(lambda: "Loading" in terminal.text() or "正在加载" in terminal.text(), timeout=2)
                terminal.send(b"\t\x7f\x7f\x7f\x7f")
                terminal.wait_for(lambda: "ALPHA-SESSION" in terminal.text(), timeout=2)
                self.assertNotIn("BETA-SESSION", terminal.text())
                terminal.send(b"\x1b")
                terminal.wait_for(lambda: "cancelled" in terminal.text() or "已取消会话" in terminal.text(), timeout=2)
                self.assertEqual(fixture.current(fixture.a), before)

    def test_completed_catalog_keeps_search_and_can_resume(self):
        """无参数；后台结果到达后保留搜索并能恢复外部工作区，返回无。"""
        fixture = Workspaces()
        with TerminalSession(prepare=fixture.setup, workspace="workspace-a") as terminal:
            with blocked_foreign_index(fixture):
                terminal.send(b"/resume\r")
                terminal.wait_for(lambda: "ALPHA-SESSION" in terminal.text(), timeout=2)
                terminal.send(b"\tBETA")
                terminal.wait_for(lambda: "Loading" in terminal.text() or "正在加载" in terminal.text(), timeout=2)
            terminal.wait_for(lambda: "BETA-SESSION" in terminal.text(), timeout=2)
            self.assertNotIn("ALPHA-SESSION", terminal.text())
            terminal.send(b"\r")
            terminal.wait_for(lambda: "Resumed session" in terminal.text() or "已恢复会话" in terminal.text())
            terminal.send(b"!printf READY > loaded-proof.txt\r")
            terminal.wait_for(lambda: (fixture.b / "loaded-proof.txt").exists())
            self.assertFalse((fixture.a / "loaded-proof.txt").exists())

    def test_cli_all_can_cancel_while_index_is_blocked(self):
        """无参数；显式全量入口在慢索引下显示加载状态并接受取消，返回无。"""
        fixture = Workspaces()
        with ExitStack() as stack:
            def prepare(root, environment, binary):
                """参数为隔离环境；在子进程启动前阻塞外部索引，返回无。"""
                fixture.setup(root, environment, binary)
                stack.enter_context(blocked_foreign_index(fixture))

            with TerminalSession(prepare=prepare, workspace="workspace-a", arguments=["resume", "--all"],
                                 ready=lambda term: "Loading" in term.text() or "正在加载" in term.text()) as terminal:
                self.assertTrue("All workspaces" in terminal.text() or "全部工作区" in terminal.text())
                terminal.send(b"\x1b")
                # 1. 【会话恢复】【取消验证】pyte 未还原独立屏幕，中文错误覆盖宽字符时检查原始输出
                terminal.wait_for(lambda: b"cancelled" in terminal.raw or "已取消会话".encode() in terminal.raw, timeout=2)
                # 2. 【会话恢复】【退出验证】索引仍阻塞时进程也必须退出，不能只打印取消消息
                terminal.wait_for(lambda: os.waitpid(terminal.pid, os.WNOHANG)[0] == terminal.pid, timeout=2)

    def test_failed_all_load_keeps_current_scope_and_can_retry(self):
        """无参数；外部索引损坏不阻止当前列表，修复索引后切换可重试，返回无。"""
        fixture = Workspaces()
        with TerminalSession(prepare=fixture.setup, workspace="workspace-a") as terminal:
            index = fixture.scope(fixture.b) / "index.json"
            contents = index.read_bytes()
            index.write_text("invalid JSON fixture")
            terminal.send(b"/resume\r")
            terminal.wait_for(lambda: "ALPHA-SESSION" in terminal.text(), timeout=2)
            terminal.send(b"\t")
            terminal.wait_for(lambda: "Load failed" in terminal.text() or "加载失败" in terminal.text(), timeout=2)
            self.assertIn("ALPHA-SESSION", terminal.text())
            terminal.send(b"\t")
            terminal.wait_for(lambda: "Current workspace" in terminal.text() or "当前工作区" in terminal.text(), timeout=2)
            index.write_bytes(contents)
            terminal.send(b"\t")
            terminal.wait_for(lambda: "BETA-SESSION" in terminal.text(), timeout=2)
            self.assertNotIn("Load failed", terminal.text())
            terminal.send(b"\x1b")
            terminal.wait_for(lambda: "cancelled" in terminal.text() or "已取消会话" in terminal.text(), timeout=2)
