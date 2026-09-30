"""【全屏视图】【终端回归】验证 Ctrl+O 全屏：输入框固定底部、滚动、点击展开、概览跳转与浮动标题。"""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import re
import threading
import unittest

from terminal import TerminalSession

ENTER_MOUSE = b"\x1b[?1000h"
LEAVE_MOUSE = b"\x1b[?1000l"


class TurnModel:
    """每轮回复一段长思考与多行正文，正文带上用户问题便于定位。"""

    def __init__(self):
        """创建本地模型替身；无参数，返回无。"""

        class Handler(BaseHTTPRequestHandler):
            """按最后一条用户消息生成回复。"""

            def log_message(self, *_args):
                """忽略访问日志；参数为日志内容，返回无。"""

            def do_POST(self):
                """返回思考与正文；无参数，返回无。"""
                request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                text = "\n".join(str(m.get("content")) for m in request["messages"] if m.get("role") == "user")
                names = re.findall(r"\b(ALPHA|BRAVO|CHARLIE|DELTA)\b", text)
                tag = names[-1] if names else "Q"
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Connection", "close")
                self.end_headers()
                thinking = "".join(f"THINK-{tag}-{index}\n" for index in range(12))
                answer = "".join(f"ANSWER-{tag}-{index}\n\n" for index in range(8))
                for delta in ({"reasoning_content": thinking}, {"content": answer}):
                    chunk = {"choices": [{"index": 0, "delta": delta, "finish_reason": None}]}
                    self.wfile.write(("data: " + json.dumps(chunk) + "\n\n").encode())
                done = {"choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]}
                self.wfile.write(("data: " + json.dumps(done) + "\n\ndata: [DONE]\n\n").encode())

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
        """返回思考按完整模式折叠展示的合成配置；无参数。"""
        return {"active_provider": "fixture", "providers": [{"id": "fixture", "display_name": "Fixture",
            "base_url": f"http://127.0.0.1:{self.server.server_port}/v1", "api_key": "local-fixture",
            "models": ["fixture"], "default_model": "fixture"}], "tools": {"enabled": False},
            "session": {"auto_title_enabled": False}, "skills": {"enabled": False},
            "memory": {"enabled": False}, "display": {"reasoning": "full"},
            "permission": {"tui_mode": "yolo", "cli_mode": "yolo"}}


def click(terminal, column, row, kind=0):
    """在 0 起始坐标发送 SGR 鼠标按下与抬起；kind 为按键编码，返回无。"""
    terminal.send(f"\x1b[<{kind};{column + 1};{row + 1}M".encode())
    if kind < 64:
        terminal.send(f"\x1b[<{kind};{column + 1};{row + 1}m".encode())


def lines(terminal):
    """返回当前屏幕行。"""
    return terminal.text().splitlines()


class FullscreenTests(unittest.TestCase):
    """真实终端中完成一次全屏浏览与继续对话。"""

    def test_fullscreen_browse_and_continue_conversation(self):
        """全屏内滚动、展开、跳转、继续提问，退出后恢复主屏；返回无。"""
        with TurnModel() as model, TerminalSession(model.config(), columns=100, rows=30) as terminal:
            for name in ["ALPHA", "BRAVO", "CHARLIE"]:
                terminal.send(name.encode() + b"\r")
                terminal.wait_for(lambda: f"ANSWER-{name}-7" in terminal.text(), timeout=15)
            # 1. 进入全屏：顶部浮动标题、鼠标捕获开启、输入框贴底
            mark = len(terminal.raw)
            terminal.send(b"\x0f")
            terminal.wait_for(lambda: "3/3" in lines(terminal)[0], timeout=5)
            self.assertIn(ENTER_MOUSE, bytes(terminal.raw[mark:]))
            screen = lines(terminal)
            self.assertIn("CHARLIE", screen[0])
            self.assertTrue(any("yolo" in line for line in screen[-3:]), "底栏应固定在最后几行")
            # 2. 点击概览轨道首个标记，跳到第一条消息，浮动标题随之切换
            rail = 97
            rail_rows = [row for row, line in enumerate(screen) if len(line) > rail and line[rail] in "╶━"]
            self.assertEqual(len(rail_rows), 3, screen)
            click(terminal, rail, rail_rows[0])
            terminal.wait_for(lambda: "1/3" in lines(terminal)[0] and "ALPHA" in lines(terminal)[0])
            # 3. 点击折叠的思考段展开，正文出现思考内容
            thought = next(row for row, line in enumerate(lines(terminal)) if "Thought" in line or "思考" in line)
            click(terminal, 4, thought)
            terminal.wait_for(lambda: "THINK-ALPHA-6" in terminal.text())
            # 4. 滚轮向下离开该位置，Ctrl+↓ 回到底部
            click(terminal, 10, 5, kind=65)
            terminal.send(b"\x1b[1;5B")
            terminal.wait_for(lambda: "3/3" in lines(terminal)[0])
            # 5. 全屏内继续提问，回复出现在正文区，全屏保持
            terminal.send(b"DELTA\r")
            terminal.wait_for(lambda: "ANSWER-DELTA-7" in terminal.text(), timeout=15)
            terminal.wait_for(lambda: "4/4" in lines(terminal)[0])
            # 6. 退出全屏：关闭鼠标捕获，主屏恢复，浮动标题消失
            mark = len(terminal.raw)
            terminal.send(b"\x0f")
            terminal.wait_for(lambda: LEAVE_MOUSE in bytes(terminal.raw[mark:]), timeout=5)
            terminal.wait_for(lambda: "ANSWER-DELTA-7" in terminal.text() and "4/4" not in terminal.text())
            terminal.send(b"!printf EXIT-OK\r")
            terminal.wait_for(lambda: "EXIT-OK" in terminal.text())


if __name__ == "__main__":
    unittest.main()
