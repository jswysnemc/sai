"""【终端侧栏】【直接切换回归】两个会话可独立发送，切换不取消原轮次。"""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import subprocess
import threading
import unittest

from terminal import TerminalSession
from test_fullscreen import click, lines


class ParallelSessionModel:
    """首会话挂起，第二会话独立完成，用于证明切换并非等待或只读。"""

    def __init__(self):
        """无参数；返回无，创建独立请求标记与首轮释放闸门。"""
        self.first_started = threading.Event()
        self.second_started = threading.Event()
        self.release_first = threading.Event()
        model = self

        class Handler(BaseHTTPRequestHandler):
            """按用户消息区分两个会话，只响应本地测试请求。"""

            def log_message(self, *_args):
                """参数为访问日志；返回无，不输出常规请求日志。"""

            def do_POST(self):
                """无参数；返回无，第一会话等待释放，第二会话立即完成。"""
                request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                text = str([message.get("content") for message in request["messages"]
                            if message.get("role") == "user"])
                first = "FIRST-SESSION-PROMPT" in text
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Connection", "close")
                self.end_headers()
                if first:
                    model.first_started.set()
                    model.release_first.wait(25)
                else:
                    model.second_started.set()
                answer = "FIRST-SESSION-ANSWER" if first else "SECOND-SESSION-ANSWER"
                chunk = {"choices": [{"index": 0, "delta": {"content": answer}, "finish_reason": None}]}
                self.wfile.write(("data: " + json.dumps(chunk) + "\n\ndata: [DONE]\n\n").encode())
                self.wfile.flush()

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def config(self):
        """无参数；返回使用本地模型、关闭自动命名的测试配置。"""
        from test_fullscreen import TurnModel
        config = TurnModel.config(self)
        config.setdefault("display", {})["fullscreen"] = True
        return config

    def close(self):
        """无参数；返回无，释放挂起请求并关闭本地服务。"""
        self.release_first.set()
        self.server.shutdown()
        self.server.server_close()


class SidebarSessionTests(unittest.TestCase):
    """使用真实伪终端验证输入、执行和草稿归属。"""

    def test_direct_switch_can_send_while_previous_agent_keeps_running(self):
        """无参数；返回无，第二会话无需等待首轮结束即可发送，两个 Agent 均完成原任务。"""
        def prepare(root, environment, binary):
            """参数为隔离目录、环境和程序；返回无，创建目标会话及起始会话。"""
            for title in ["TARGET-SESSION", "FIRST-SESSION"]:
                subprocess.run([str(binary), "sessions", "new", title], cwd=root,
                               env=environment, capture_output=True, check=True)

        model = ParallelSessionModel()
        try:
            with TerminalSession(model.config(), columns=160, rows=36, prepare=prepare,
                                 arguments=["--lang", "en-US"]) as terminal:
                terminal.wait_for(lambda: "TARGET-SESSION" in terminal.text())
                terminal.send(b"FIRST-SESSION-PROMPT\r")
                terminal.wait_for(model.first_started.is_set, timeout=10)
                terminal.send(b"FIRST-SESSION-DRAFT")
                terminal.wait_for(lambda: "FIRST-SESSION-DRAFT" in terminal.text())
                row = next(index for index, line in enumerate(lines(terminal)) if "TARGET-SESSION" in line[120:])
                click(terminal, 130, row)
                terminal.wait_for(lambda: any("Current" in line[120:] and "TARGET" in line[120:] for line in lines(terminal)))
                self.assertNotIn("Read-only session view", terminal.text())
                self.assertNotIn("FIRST-SESSION-DRAFT", terminal.text())
                # 1. 【终端侧栏】【独立提交】首会话仍挂起时，目标会话直接收到并完成自己的消息
                terminal.send(b"SECOND-SESSION-PROMPT\r")
                terminal.wait_for(model.second_started.is_set, timeout=10)
                terminal.wait_for(lambda: "SECOND-SESSION-ANSWER" in terminal.text(), timeout=10)
                self.assertFalse(model.release_first.is_set())
                terminal.send(b"SECOND-SESSION-DRAFT")
                terminal.wait_for(lambda: "SECOND-SESSION-DRAFT" in terminal.text())
                # 2. 【终端侧栏】【原轮接管】返回运行中会话保留原草稿，释放后必须继续原轮而非重新发送
                row = next(index for index, line in enumerate(lines(terminal)) if "FIRST-SESSION" in line[120:])
                click(terminal, 130, row)
                terminal.wait_for(lambda: "FIRST-SESSION-DRAFT" in terminal.text())
                self.assertNotIn("SECOND-SESSION-DRAFT", terminal.text())
                model.release_first.set()
                terminal.wait_for(lambda: "FIRST-SESSION-ANSWER" in terminal.text(), timeout=10)
                self.assertIn("FIRST-SESSION-DRAFT", terminal.text())
                # 3. 【终端侧栏】【空闲切换】轮次完成后点击仍直接切换，并恢复目标会话已有草稿
                row = next(index for index, line in enumerate(lines(terminal)) if "TARGET-SESSION" in line[120:])
                click(terminal, 130, row)
                terminal.wait_for(lambda: "SECOND-SESSION-DRAFT" in terminal.text())
                self.assertIn("SECOND-SESSION-ANSWER", terminal.text())
                self.assertNotIn("FIRST-SESSION-DRAFT", terminal.text())
        finally:
            model.close()


if __name__ == "__main__":
    unittest.main()
