"""【SSH 征询】【终端回归】验证密码卡片挂在受管输入区，取消后恢复且不回显。"""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import socket
import threading
import unittest

from terminal import TerminalSession


def local_sshd_available():
    """检测本机 22 端口是否有 SSH 服务；无参数，返回布尔值。"""
    try:
        with socket.create_connection(("127.0.0.1", 22), timeout=1) as connection:
            return connection.recv(4).startswith(b"SSH-")
    except OSError:
        return False


class SshToolModel:
    """依次返回加载与调用 ssh_run_command 的工具调用，最后返回文本。"""

    def __init__(self):
        """创建本地模型替身；无参数，返回无。"""

        class Handler(BaseHTTPRequestHandler):
            """按已有工具消息数量决定下一步调用。"""

            def log_message(self, *_args):
                """忽略访问日志；参数为日志内容，返回无。"""

            def do_POST(self):
                """返回一次完整的流式响应；返回无。"""
                request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                tools = sum(1 for message in request["messages"] if message["role"] == "tool")
                if tools == 0:
                    call = {"name": "load", "arguments": json.dumps({"type": "tool", "keywords": ["ssh_run_command"]})}
                elif tools == 1:
                    call = {"name": "invoke_tool", "arguments": json.dumps({
                        "tool_name": "ssh_run_command", "arguments": {"host_id": "fixture", "command": "uptime"}})}
                else:
                    call = None
                delta = {"tool_calls": [{"index": 0, "id": f"call-{tools}", "type": "function", "function": call}]} if call else {"content": "SSH-DONE"}
                finish = "tool_calls" if call else "stop"
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Connection", "close")
                self.end_headers()
                chunk = {"choices": [{"index": 0, "delta": delta, "finish_reason": finish}]}
                self.wfile.write(("data: " + json.dumps(chunk) + "\n\ndata: [DONE]\n\n").encode())

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)

    def __enter__(self):
        """启动服务；返回实例。"""
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        return self

    def __exit__(self, *_args):
        """关闭服务；参数为异常信息，返回无。"""
        self.server.shutdown()
        self.server.server_close()

    def config(self):
        """返回指向本机 sshd、用户名不存在的合成配置；无参数。"""
        return {"active_provider": "fixture", "providers": [{"id": "fixture", "display_name": "Fixture",
            "base_url": f"http://127.0.0.1:{self.server.server_port}/v1", "api_key": "local-fixture",
            "models": ["fixture"], "default_model": "fixture"}], "tools": {"enabled": True},
            "session": {"auto_title_enabled": False}, "skills": {"enabled": False},
            "memory": {"enabled": False}, "permission": {"tui_mode": "yolo", "cli_mode": "yolo"},
            "ssh": {"hosts": [{"id": "fixture", "label": "fixture-host", "hostname": "127.0.0.1",
                               "port": 22, "username": "sai-no-such-user"}]}}


def isolate_home(root, environment, _binary):
    """把 HOME 指向临时目录，避免写入用户 known_hosts；返回无。"""
    home = root / "home"
    home.mkdir()
    environment["HOME"] = str(home)


@unittest.skipUnless(local_sshd_available(), "local sshd on port 22 is required")
class SshPromptTests(unittest.TestCase):
    """通过真实终端验证 SSH 征询卡片的布局与清理。"""

    def test_password_card_stays_above_composer_and_clears_on_cancel(self):
        """卡片位于输入行上方、底栏不被覆盖；键入不回显，Esc 取消后卡片消失；返回无。"""
        with SshToolModel() as model, TerminalSession(model.config(), columns=100, rows=30, prepare=isolate_home) as terminal:
            terminal.send(b"connect\r")
            terminal.wait_for(lambda: "fixture-host" in terminal.text() and ("登录密码" in terminal.text() or "login password" in terminal.text()), timeout=25)
            lines = terminal.text().splitlines()
            card = next(index for index, line in enumerate(lines) if "fixture-host" in line)
            composer = next(index for index, line in enumerate(lines) if "→" in line)
            self.assertLess(card, composer)
            self.assertTrue(any("yolo" in line for line in lines[composer + 1:]), "底栏必须保留在输入行下方")
            terminal.send(b"hunter2")
            terminal.wait_for(lambda: "已输入" in terminal.text() or "Typed" in terminal.text())
            self.assertNotIn("hunter2", terminal.text(history=True))
            terminal.send(b"\x1b")
            terminal.wait_for(lambda: "SSH-DONE" in terminal.text(), timeout=15)
            self.assertNotIn("fixture-host", terminal.text())
            self.assertNotIn("hunter2", terminal.text(history=True))


if __name__ == "__main__":
    unittest.main()
