"""【终端队列】【恢复回归】失败或中断后保留未执行消息及长文本附件，按原顺序继续发送。"""

import json
import unittest

from mock_queue_model import QueueModel
from terminal import TerminalSession


class QueueRecoveryTests(unittest.TestCase):
    """通过真实按键、HTTP 请求和队列管理面板验证消息不丢失。"""

    def assert_queue_recovered(self, interrupt):
        """interrupt 选择失败或中断路径；验证剩余消息及附件后返回无。"""
        with QueueModel(interrupt) as model, TerminalSession(model.config(), columns=100, rows=30) as terminal:
            terminal.send(b"INITIAL\r")
            terminal.wait_for(model.started.is_set)
            terminal.wait_for(lambda: "INITIAL-START" in terminal.text())
            terminal.send(b"QUEUED_FAILURE\t")
            terminal.pump(.2)
            long_text = "QUEUED_LONG " + "complete-payload " * 500
            terminal.send(b"\x1b[200~" + long_text.encode() + b"\x1b[201~")
            terminal.pump(.2)
            terminal.send(b"\t")
            terminal.pump(.2)
            terminal.send(b"QUEUED_LAST\t")
            terminal.pump(.2)
            self.assertEqual(model.received, ["INITIAL"])
            model.release.set()
            terminal.wait_for(model.queue_started.is_set)
            if interrupt:
                terminal.wait_for(lambda: "QUEUED-START" in terminal.text())
                terminal.send(b"\x03")
            else:
                terminal.wait_for(lambda: "QUEUE-FAILURE-FIXTURE" in terminal.visible_text(history=True))
            terminal.wait_for(lambda: any(label in terminal.text(history=True) for label in
                                         ["已保留 2", "kept 2", "已丢弃 2", "discarded 2"]))
            history = terminal.text(history=True)
            self.assertNotIn("已丢弃 2", history)
            self.assertNotIn("discarded 2", history)
            self.assertEqual(model.received, ["INITIAL", "QUEUED_FAILURE"])
            # 1. 【终端队列】【显式继续】进入管理面板发送首条保留消息，后续消息按原顺序执行
            terminal.send(b"\x1b[1;5A")
            terminal.wait_for(lambda: "QUEUED_LAST" in terminal.text())
            terminal.send(b"\x1b[H")
            terminal.pump(.1)
            terminal.send(b"s")
            terminal.wait_for(lambda: "QUEUED_LAST-DONE" in terminal.text(), timeout=15)
            self.assertEqual(model.received, ["INITIAL", "QUEUED_FAILURE", "QUEUED_LONG", "QUEUED_LAST"])
            self.assertIn(long_text.strip(), json.dumps(model.requests[2], ensure_ascii=False))

    def test_failed_queued_turn_keeps_later_submissions(self):
        """排队首条返回 HTTP 错误后保留其余两条消息；无参数，返回无。"""
        self.assert_queue_recovered(False)

    def test_interrupted_queued_turn_keeps_later_submissions(self):
        """中断排队首条后保留其余两条消息；无参数，返回无。"""
        self.assert_queue_recovered(True)
