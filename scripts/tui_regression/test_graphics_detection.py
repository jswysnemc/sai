"""【终端图片】【协议识别】验证不依赖终端名称的 Kitty 能力识别。"""

import json
import time
import unittest

from terminal import TerminalSession
from test_composer_images import active_placements, prepare_history


def prepare_herdr(root, environment, binary):
    """【终端图片】【复现环境】参数为隔离目录、环境及程序；准备无 Kitty 标识的图片历史，返回无。"""
    prepare_history(root, environment, binary)
    # 1. 【终端图片】【最小样本】一张附件即可验证协议识别，避免依赖多图布局
    history = root / "state/sai/input-history.jsonl"
    entry = json.loads(history.read_text())
    entry["attachments"] = entry["attachments"][:1]
    entry["text"] = entry["attachments"][0]["marker"]
    history.write_text(json.dumps(entry) + "\n")
    for key in ("KITTY_WINDOW_ID", "WEZTERM_PANE", "WT_SESSION", "TMUX"):
        environment.pop(key, None)
    environment.update(TERM="xterm-256color", TERM_PROGRAM="herdr")


class ReplyTerminal(TerminalSession):
    """【终端图片】【应答控制】复用真实伪终端，控制成功、失败、分段响应及用户输入。"""

    def __init__(self, *args, status="OK", split=False, early_input=b"", wrong_id=False, **kwargs):
        """参数为会话配置及应答方式；保存应答状态并创建终端，返回无。"""
        self.status = status
        self.split = split
        self.early_input = early_input
        self.wrong_id = wrong_id
        self.queries = []
        super().__init__(*args, **kwargs)

    def reply_graphics_query(self, control):
        """参数 control 为实际查询；记录并发送指定应答，分块之间短暂等待，返回无。"""
        self.queries.append(control)
        if self.early_input:
            self.send(self.early_input)
        if self.status is None:
            return
        response_id = str(int(control["i"]) + int(self.wrong_id))
        reply = f"\x1b_Gi={response_id};{self.status}\x1b\\".encode()
        for part in ([bytes([byte]) for byte in reply] if self.split else [reply]):
            self.send(part)
            if self.split:
                time.sleep(.002)


class GraphicsDetectionTests(unittest.TestCase):
    """【终端图片】【能力应答】使用真实程序与伪终端验证协议选择结果。"""

    def test_herdr_graphics_without_kitty_environment(self):
        """无参数；普通终端环境应根据 Kitty 查询应答展示附件，返回无。"""
        for view in ("inline", "fullscreen"):
            with self.subTest(view=view), ReplyTerminal(
                    prepare=prepare_herdr, kitty_graphics=True,
                    arguments=["--lang", "en-US", "--view", view]) as terminal:
                terminal.send(b"\x1b[A")
                terminal.wait_for(lambda: len(active_placements(terminal.raw)) == 1, timeout=2)
                self.assertIn("[image 1 640x480]", terminal.text())
                self.assertEqual(len(terminal.queries), 1)
                self.assertEqual(terminal.queries[0]["t"], "d")
                self.assertNotIn(";OK", terminal.text())

    def test_unknown_terminal_without_program_name(self):
        """无参数；没有终端程序名称时仍按实际应答识别图形能力，返回无。"""
        def prepare(root, environment, binary):
            """参数为隔离目录、环境及程序；去掉全部终端名称提示，返回无。"""
            prepare_herdr(root, environment, binary)
            environment.pop("TERM_PROGRAM", None)

        with ReplyTerminal(prepare=prepare, kitty_graphics=True) as terminal:
            terminal.send(b"\x1b[A")
            terminal.wait_for(lambda: len(active_placements(terminal.raw)) == 1, timeout=2)
            self.assertEqual(len(terminal.queries), 1)

    def test_failed_or_missing_response_keeps_fallback(self):
        """无参数；错误、错误编号及无应答都保持文字附件，响应不进入草稿，返回无。"""
        for status, wrong_id in [("EINVAL: unsupported", False), (None, False), ("OK", True)]:
            with self.subTest(status=status, wrong_id=wrong_id), ReplyTerminal(
                    prepare=prepare_herdr, status=status, wrong_id=wrong_id) as terminal:
                terminal.send(b"\x1b[A")
                terminal.wait_for(lambda: "[image 1 640x480]" in terminal.text())
                self.assertFalse(active_placements(terminal.raw))
                self.assertEqual(len(terminal.queries), 1)
                self.assertNotIn("EINVAL", terminal.text())
                self.assertNotIn(";OK", terminal.text())
                terminal.send(b"kept input")
                terminal.wait_for(lambda: "kept input" in terminal.text())

    def test_fragmented_response_preserves_startup_input(self):
        """无参数；逐字节应答能启用协议，探测期间的中文和编辑键仍按原顺序处理，返回无。"""
        # 1. 【终端图片】【真实输入】恢复历史后追加文字，再把 x 插到 ab 之间
        early = b"\x1b[A" + "保留ab".encode() + b"\x1b[Dx"
        with ReplyTerminal(prepare=prepare_herdr, kitty_graphics=True,
                           split=True, early_input=early) as terminal:
            terminal.wait_for(lambda: "保留axb" in terminal.text())
            terminal.wait_for(lambda: len(active_placements(terminal.raw)) == 1, timeout=2)
            self.assertEqual(len(terminal.queries), 1)
            self.assertNotIn(";OK", terminal.text())

    def test_known_terminal_keeps_existing_protocol_without_probe(self):
        """无参数；现有 Kitty 环境沿用已知协议，不增加启动查询，返回无。"""
        with ReplyTerminal(prepare=prepare_history, kitty_graphics=True) as terminal:
            terminal.send(b"\x1b[A")
            terminal.wait_for(lambda: len(active_placements(terminal.raw)) == 3)
            self.assertEqual(terminal.queries, [])

    def test_timeout_preserves_startup_input(self):
        """无参数；终端不应答时查询有界结束，期间收到的用户文本完整保留，返回无。"""
        with ReplyTerminal(prepare=prepare_herdr, status=None, early_input=b"kept before timeout") as terminal:
            terminal.wait_for(lambda: "kept before timeout" in terminal.text())
            self.assertEqual(len(terminal.queries), 1)

    def test_tmux_does_not_probe_without_passthrough(self):
        """无参数；没有图形透传支持的 tmux 环境不发送主动查询，返回无。"""
        def prepare(root, environment, binary):
            """参数为隔离目录、环境及程序；增加 tmux 标记，返回无。"""
            prepare_herdr(root, environment, binary)
            environment["TMUX"] = "/tmp/sai-test-tmux,1,0"

        with ReplyTerminal(prepare=prepare, kitty_graphics=True) as terminal:
            terminal.send(b"\x1b[A")
            terminal.wait_for(lambda: "[image 1 640x480]" in terminal.text())
            self.assertFalse(active_placements(terminal.raw))
            self.assertEqual(terminal.queries, [])


if __name__ == "__main__":
    unittest.main()
