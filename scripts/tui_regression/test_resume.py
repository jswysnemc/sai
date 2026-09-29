"""【会话恢复】【终端回归】验证工作区过滤、恢复与实际进程目录。"""

import hashlib
import json
import subprocess
import unittest

from terminal import TerminalSession
from mock_resume_model import ResumeModel


class Workspaces:
    """使用真实 CLI 创建隔离工作区，并记录会话身份。"""

    def setup(self, root, environment, binary):
        """参数为临时根、环境和程序；创建双工作区会话，返回无。"""
        self.root, self.environment, self.binary = root, environment, binary
        self.a, self.b = root / "workspace-a", root / "workspace-b"
        self.sessions = {}
        for path, title in [(self.a, "ALPHA-SESSION"), (self.b, "BETA-SESSION")]:
            path.mkdir()
            result = subprocess.run([str(binary), "sessions", "new", title], cwd=path, env=environment, capture_output=True, timeout=20)
            if result.returncode:
                raise AssertionError(result.stderr.decode())
            scope = self.scope(path)
            self.sessions[path.name] = json.loads((scope / "index.json").read_text())[0]["id"]
            (path / f"{path.name}-file.txt").write_text(path.name)

    def scope(self, path):
        """path 为规范工作区路径；返回真实会话索引目录。"""
        identifier = hashlib.sha256(str(path.resolve()).encode()).hexdigest()[:16]
        return self.root / "state/sai/sessions/workspaces" / identifier

    def current(self, path):
        """path 为工作区；返回当前会话指针。"""
        return (self.scope(path) / "current").read_text().strip()


def open_picker(terminal):
    """terminal 为会话；打开默认工作区选择器，返回无。"""
    terminal.send(b"/resume\r")
    terminal.wait_for(lambda: "Current workspace" in terminal.text() or "当前工作区" in terminal.text())


class ResumeTests(unittest.TestCase):
    """通过真实终端和文件系统验证恢复行为。"""

    def test_legacy_session_can_locate_workspace_in_tui(self):
        """旧会话缺少路径时，在统一表单定位原目录后恢复并补录路径；返回无。"""
        fixture = Workspaces()
        with TerminalSession(prepare=fixture.setup, workspace="workspace-a") as terminal:
            metadata = fixture.scope(fixture.b) / "workspace.json"
            metadata.unlink()
            open_picker(terminal)
            terminal.send(b"\tBETA\r")
            terminal.wait_for(lambda: "LOCATE WORKSPACE" in terminal.text() or "定位原工作区" in terminal.text())
            terminal.send(b"\r" + str(fixture.b).encode() + b"\rs")
            terminal.wait_for(lambda: "Resumed session" in terminal.text() or "已恢复会话" in terminal.text())
            self.assertEqual(json.loads(metadata.read_text())["path"], str(fixture.b))
            terminal.send(b"!printf LEGACY > legacy-proof.txt\r")
            terminal.wait_for(lambda: (fixture.b / "legacy-proof.txt").exists())

    def test_cli_all_starts_in_grouped_workspace_view(self):
        """CLI --all 直接打开全量分组视图，选择后恢复目标目录；返回无。"""
        fixture = Workspaces()
        with TerminalSession(prepare=fixture.setup, workspace="workspace-a", arguments=["resume", "--all"],
                             ready=lambda term: "All workspaces" in term.text() or "全部工作区" in term.text()) as terminal:
            self.assertIn("ALPHA-SESSION", terminal.text())
            self.assertIn("BETA-SESSION", terminal.text())
            terminal.send(b"BETA\r")
            terminal.wait_for(lambda: "auto" in terminal.text())
            terminal.send(b"!printf CLI > all-proof.txt\r")
            terminal.wait_for(lambda: (fixture.b / "all-proof.txt").exists())

    def test_corrupt_target_rolls_back_after_directory_switch(self):
        """目标数据库无法打开时，目录与原活动会话均保持可用；返回无。"""
        fixture = Workspaces()
        with TerminalSession(prepare=fixture.setup, workspace="workspace-a") as terminal:
            target = fixture.scope(fixture.b) / "data" / fixture.sessions["workspace-b"]
            target.mkdir(parents=True, exist_ok=True)
            (target / "conversation.db").write_bytes(b"invalid database fixture")
            before = fixture.current(fixture.a)
            terminal.send(("/resume " + fixture.sessions["workspace-b"] + "\r").encode())
            terminal.wait_for(lambda: "database" in terminal.text())
            terminal.send(b"!printf ROLLBACK > rollback-proof.txt\r")
            terminal.wait_for(lambda: (fixture.a / "rollback-proof.txt").exists())
            self.assertFalse((fixture.b / "rollback-proof.txt").exists())
            self.assertEqual(fixture.current(fixture.a), before)

    def test_resume_restores_history_and_rebinds_model_tools(self):
        """恢复历史后模型文件工具必须使用目标目录；返回无。"""
        fixture = Workspaces()

        def prepare(root, environment, binary):
            """创建工作区并通过真实模型链路写入历史，参数为隔离环境；返回无。"""
            fixture.setup(root, environment, binary)
            result = subprocess.run([str(binary), "ask", "BETA-HISTORY-QUESTION"], cwd=fixture.b,
                                    env=environment, capture_output=True, timeout=30)
            if result.returncode:
                raise AssertionError(result.stderr.decode())

        with ResumeModel() as model, TerminalSession(config=model.config(), prepare=prepare, workspace="workspace-a") as terminal:
            terminal.send(("/resume " + fixture.sessions["workspace-b"] + "\r").encode())
            terminal.wait_for(lambda: "RESUMED-HISTORY-ANSWER" in terminal.text())
            self.assertIn("BETA-HISTORY-QUESTION", terminal.text())
            terminal.pump(.4)
            terminal.send(b"WRITE-TARGET")
            terminal.pump(.15)
            terminal.send(b"\r")
            terminal.wait_for(lambda: (fixture.b / "model-proof.txt").exists(), timeout=20)
            self.assertEqual((fixture.b / "model-proof.txt").read_text(), "TARGET-WORKSPACE")
            self.assertFalse((fixture.a / "model-proof.txt").exists())
            self.assertTrue(any("BETA-HISTORY-QUESTION" in json.dumps(request) and "WRITE-TARGET" in json.dumps(request)
                                for request in model.requests))

    def test_default_scope_toggle_and_cross_workspace_shell(self):
        """默认隐藏异地会话，切换选择后 Shell 必须写入目标工作区；返回无。"""
        fixture = Workspaces()
        with TerminalSession(prepare=fixture.setup, workspace="workspace-a") as terminal:
            open_picker(terminal)
            self.assertIn("ALPHA-SESSION", terminal.text())
            self.assertNotIn("BETA-SESSION", terminal.text())
            terminal.send(b"\t")
            terminal.wait_for(lambda: "BETA-SESSION" in terminal.text())
            self.assertIn("workspace-a", terminal.text())
            self.assertIn("workspace-b", terminal.text())
            terminal.send(b"BETA\r")
            terminal.wait_for(lambda: "Resumed session" in terminal.text() or "已恢复会话" in terminal.text())
            terminal.send(b"!printf RESUMED > cwd-proof.txt\r")
            terminal.wait_for(lambda: (fixture.b / "cwd-proof.txt").exists())
            self.assertFalse((fixture.a / "cwd-proof.txt").exists())
            terminal.pump(.3)
            terminal.send(b"@workspace-b")
            terminal.wait_for(lambda: "workspace-b-file.txt" in terminal.text())
            self.assertNotIn("workspace-a-file.txt", terminal.text())

    def test_cancel_preserves_directory_and_current_session(self):
        """切换范围、搜索和缩放后取消，不改变目录或会话指针；返回无。"""
        fixture = Workspaces()
        with TerminalSession(prepare=fixture.setup, workspace="workspace-a") as terminal:
            before = fixture.current(fixture.a)
            open_picker(terminal)
            terminal.send(b"\tBETA")
            terminal.wait_for(lambda: "BETA-SESSION" in terminal.text())
            terminal.resize(44, 12)
            terminal.pump(.2)
            terminal.send(b"\x1b")
            terminal.wait_for(lambda: "cancelled" in terminal.text() or "已取消会话" in terminal.text())
            terminal.send(b"!printf LOCAL > cancelled-proof.txt\r")
            terminal.wait_for(lambda: (fixture.a / "cancelled-proof.txt").exists())
            self.assertEqual(fixture.current(fixture.a), before)
            self.assertFalse((fixture.b / "cancelled-proof.txt").exists())

    def test_missing_workspace_keeps_existing_repl_usable(self):
        """目标目录被删除时恢复失败，原会话仍可执行命令；返回无。"""
        fixture = Workspaces()
        with TerminalSession(prepare=fixture.setup, workspace="workspace-a") as terminal:
            (fixture.b / "workspace-b-file.txt").unlink()
            fixture.b.rmdir()
            terminal.send(("/resume " + fixture.sessions["workspace-b"] + "\r").encode())
            terminal.wait_for(lambda: "Workspace unavailable" in terminal.text())
            terminal.send(b"!printf SAFE > failed-proof.txt\r")
            terminal.wait_for(lambda: (fixture.a / "failed-proof.txt").exists())
            self.assertEqual(fixture.current(fixture.a), fixture.sessions["workspace-a"])

    def test_cli_explicit_workspace_resolves_duplicate_id(self):
        """两个工作区同名 ID 时，CLI 显式目录必须恢复对应会话；返回无。"""
        fixture = Workspaces()
        arguments = ["resume", "shared", "--workspace", "../workspace-b"]

        def prepare(root, environment, binary):
            """建立同名索引，参数为隔离环境；返回无。"""
            fixture.setup(root, environment, binary)
            for path in [fixture.a, fixture.b]:
                index = fixture.scope(path) / "index.json"
                entries = json.loads(index.read_text())
                entries[0]["id"] = "shared"
                index.write_text(json.dumps(entries))
                (fixture.scope(path) / "current").write_text("shared\n")

        with TerminalSession(prepare=prepare, workspace="workspace-a", arguments=arguments) as terminal:
            terminal.send(b"!printf BETA > duplicate-proof.txt\r")
            terminal.wait_for(lambda: (fixture.b / "duplicate-proof.txt").exists())
            self.assertFalse((fixture.a / "duplicate-proof.txt").exists())
