"""【终端】【上下文编辑回归】单条件策略、窄屏面板与输入恢复。"""
import json
import unittest

from terminal import TerminalSession


class ContextEditTests(unittest.TestCase):
    """用真实伪终端检查编辑、保存及取消。"""

    def test_session_saves_selected_mode_and_preset(self):
        """无参数；比例默认值保存为 80%，随后切换到预留 8k，恢复默认取消本次保存。"""
        for fullscreen in (False, True):
            arguments = ["--lang", "en-US", "--view", "fullscreen" if fullscreen else "inline"]
            with self.subTest(fullscreen=fullscreen), TerminalSession(arguments=arguments, columns=100, rows=32) as terminal:
                before = len(terminal.raw)
                terminal.send(b"/context edit\r")
                terminal.wait_for(lambda: "Usage ratio" in terminal.text())
                self.assertNotIn(b"\x1b[?1049h", terminal.raw[before:])
                self.assertNotIn(b"\x1b[?1049l", terminal.raw[before:])
                terminal.send(b"e80\r\r")
                terminal.wait_for(lambda: "policy saved" in terminal.text())
                policies = list((terminal.root / "state").rglob("compaction-policy.json"))
                self.assertEqual(len(policies), 1)
                self.assertEqual(json.loads(policies[0].read_text())["compaction_mode"], "ratio")
                self.assertEqual(json.loads(policies[0].read_text())["compaction_ratio"], .8)
                terminal.send(b"/context edit\r")
                terminal.wait_for(lambda: "80%" in terminal.text())
                terminal.send(b"\t1\r")
                terminal.wait_for(lambda: "AUTO-COMPACT" not in terminal.text())
                self.assertIn("policy saved", terminal.text())
                policy = json.loads(policies[0].read_text())
                self.assertEqual(policy["compaction_mode"], "reserve")
                self.assertEqual(policy["compaction_reserve_tokens"], 8000)
                terminal.send(b"/context edit\r")
                terminal.wait_for(lambda: "Safety reserve" in terminal.text())
                terminal.resize(36, 12)
                terminal.wait_for(lambda: "Safety reserve" in terminal.text())
                terminal.send(b"rq")
                terminal.wait_for(lambda: "settings cancelled" in terminal.text())
                self.assertEqual(json.loads(policies[0].read_text())["compaction_reserve_tokens"], 8000)
                terminal.send(b"AFTER-EDIT")
                terminal.wait_for(lambda: "AFTER-EDIT" in terminal.text())

    def test_idle_panel_does_not_repaint(self):
        """无参数；数值输入实时预览，空闲不产生周期性清屏输出。"""
        with TerminalSession(arguments=["--lang", "en-US"], columns=100, rows=32) as terminal:
            terminal.send(b"/context edit\r")
            terminal.wait_for(lambda: "Usage ratio" in terminal.text())
            terminal.send(b"e85")
            terminal.wait_for(lambda: "85|" in terminal.text())
            before = len(terminal.raw)
            terminal.pump(.4)
            self.assertEqual(len(terminal.raw), before)
            terminal.send(b"\r\r")
            terminal.wait_for(lambda: "policy saved" in terminal.text())
            policies = list((terminal.root / "state").rglob("compaction-policy.json"))
            self.assertEqual(json.loads(policies[0].read_text())["compaction_ratio"], .85)
