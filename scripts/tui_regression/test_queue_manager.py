"""【用户队列】【终端回归】验证示例中的限高列表、调序、恢复和鼠标操作。"""

import unittest

from terminal import TerminalSession
from mock_activity_model import ActivityModel
from test_fullscreen import click, lines
from test_input_mouse import locate


def enqueue(terminal, text):
    """参数为终端和消息；使用 Enter 排入下轮，返回无。"""
    terminal.send(text.encode() + b"\r")
    terminal.wait_for(lambda: text in terminal.text())


def manager_rows(terminal):
    """参数为终端；返回队列可见消息行，忽略草稿和聊天正文。"""
    return [line for line in lines(terminal) if "#" in line and ("Next turn" in line or "Steer" in line)]


class QueueManagerTests(unittest.TestCase):
    """模型保持首轮暂停，队列操作期间不访问外部服务。"""

    def test_reorder_top_timing_delete_restore_and_edit(self):
        """无参数；键盘完成调序、置顶、时机切换、删除确认、恢复和取回编辑，返回无。"""
        with ActivityModel() as model:
            with TerminalSession(model.config(), columns=100, rows=32,
                                 arguments=["--lang", "en-US"]) as terminal:
                terminal.send(b"START\r")
                terminal.wait_for(model.ready.is_set, timeout=15)
                for index in range(12):
                    enqueue(terminal, f"QUEUE-{index:02}")
                terminal.send(b"\x1b[1;5A")
                terminal.wait_for(lambda: "Queue manager" in terminal.text())
                terminal.wait_for(lambda: "› #12" in terminal.text())
                self.assertLessEqual(len(manager_rows(terminal)), 5)
                self.assertIn("QUEUE-11", terminal.text())
                terminal.send(b"\x1b[1;2A")
                terminal.wait_for(lambda: any("› #11" in line and "QUEUE-11" in line for line in manager_rows(terminal)))
                terminal.send(b"t")
                terminal.wait_for(lambda: any("› #1" in line and "QUEUE-11" in line for line in manager_rows(terminal)))
                terminal.send(b"\t")
                terminal.wait_for(lambda: any("› #1" in line and "Steer" in line for line in manager_rows(terminal)))
                terminal.send(b"d")
                terminal.wait_for(lambda: "Delete this message?" in terminal.text())
                terminal.send(b"\x1b")
                terminal.wait_for(lambda: "Delete this message?" not in terminal.text())
                self.assertIn("Queue manager (12)", terminal.text())
                terminal.send(b"dd")
                terminal.wait_for(lambda: "Queue manager (11)" in terminal.text())
                terminal.send(b"u")
                terminal.wait_for(lambda: "Queue manager (12)" in terminal.text())
                self.assertTrue(any("› #1" in line and "QUEUE-11" in line and "Steer" in line for line in manager_rows(terminal)))
                terminal.send(b"\r")
                terminal.wait_for(lambda: "Queue manager" not in terminal.text())
                terminal.send(b"-EDIT\r")
                terminal.wait_for(lambda: "QUEUE-11-EDIT" in terminal.text())
                terminal.send(b"\x1bm")
                terminal.wait_for(lambda: "Queue manager (12)" in terminal.text())
                self.assertTrue(any("QUEUE-11-EDIT" in line and "Steer" in line for line in manager_rows(terminal)))

    def test_mouse_list_scroll_controls_and_input_focus(self):
        """无参数；鼠标仅滚动队列、按钮独立反色，点击输入退出管理但保留队列，返回无。"""
        with ActivityModel() as model:
            with TerminalSession(model.config(), columns=100, rows=32,
                                 arguments=["--lang", "en-US"]) as terminal:
                terminal.send(b"START\r")
                terminal.wait_for(model.ready.is_set, timeout=15)
                for index in range(9):
                    enqueue(terminal, f"MOUSE-{index:02}")
                column, row = locate(terminal, "Queued")
                click(terminal, column, row)
                terminal.wait_for(lambda: "Queue manager" in terminal.text())
                terminal.send(b"\x1b[H")
                terminal.wait_for(lambda: "MOUSE-00" in "\n".join(manager_rows(terminal)))
                column, row = locate(terminal, "MOUSE-02")
                terminal.send(f"\x1b[<35;{column + 1};{row + 1}M".encode())
                terminal.wait_for(lambda: terminal.screen.buffer[row][column].reverse)
                click(terminal, column, row)
                terminal.wait_for(lambda: any("› #3" in line for line in manager_rows(terminal)))
                column, row = locate(terminal, "T Top")
                click(terminal, column, row)
                terminal.wait_for(lambda: any("› #1" in line and "MOUSE-02" in line for line in manager_rows(terminal)))
                column, row = locate(terminal, "MOUSE-02")
                chat = lines(terminal)[:5]
                click(terminal, column, row, kind=65)
                terminal.wait_for(lambda: any("› #2" in line for line in manager_rows(terminal)))
                self.assertEqual(lines(terminal)[:5], chat)
                terminal.resize(50, 24)
                terminal.wait_for(lambda: "[ u ]" in terminal.text())
                self.assertLessEqual(len(manager_rows(terminal)), 4)
                terminal.send(b"\x1b")
                terminal.wait_for(lambda: "Queue manager" not in terminal.text())
                terminal.send(b"KEPT-DRAFT")
                terminal.wait_for(lambda: "KEPT-DRAFT" in terminal.text())
                terminal.send(b"\x1bm")
                terminal.wait_for(lambda: "Queue manager" in terminal.text())
                column, row = locate(terminal, "DRAFT")
                click(terminal, column, row)
                terminal.send(b"EDIT-")
                terminal.wait_for(lambda: "KEPT-EDIT-DRAFT" in terminal.text())
                self.assertNotIn("Queue manager", terminal.text())

    def test_ctrl_enter_queues_steer_without_changing_plain_enter(self):
        """无参数；增强键盘协议下 Ctrl+Enter 插话，普通 Enter 仍然排入下轮，返回无。"""
        with ActivityModel() as model:
            with TerminalSession(model.config(), columns=100, rows=32, kitty_keyboard=True,
                                 arguments=["--lang", "en-US"]) as terminal:
                terminal.send(b"START\r")
                terminal.wait_for(model.ready.is_set, timeout=15)
                terminal.send(b"STEER-MESSAGE\x1b[13;5u")
                terminal.wait_for(lambda: "STEER-MESSAGE" in terminal.text())
                terminal.pump(.2)
                enqueue(terminal, "NEXT-MESSAGE")
                terminal.send(b"\x1b[1;5A")
                terminal.wait_for(lambda: "Queue manager (2)" in terminal.text())
                rows = manager_rows(terminal)
                self.assertTrue(any("STEER-MESSAGE" in row and "Steer" in row for row in rows))
                self.assertTrue(any("NEXT-MESSAGE" in row and "Next turn" in row for row in rows))


if __name__ == "__main__":
    unittest.main()
