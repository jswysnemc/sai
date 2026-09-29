"""【会话恢复】【模型替身】提供历史回复与完整文件工具调用。"""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import threading


class ResumeModel:
    """只在本地处理合成请求，记录用于验证恢复历史的消息。"""

    def __init__(self):
        """创建本地模拟服务，无参数，返回无。"""
        self.requests = []
        model = self

        class Handler(BaseHTTPRequestHandler):
            """返回固定文字或一次完整文件写入调用。"""

            def log_message(self, *_args):
                """忽略访问日志，参数为日志信息；返回无。"""

            def do_POST(self):
                """记录实际模型上下文并响应；返回无。"""
                request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                model.requests.append(request)
                last = request["messages"][-1]
                if last["role"] == "user" and "WRITE-TARGET" in str(last.get("content", "")):
                    delta = {"tool_calls": [{"index": 0, "id": "resume-write", "type": "function", "function": {
                        "name": "write_file", "arguments": json.dumps({"path": "model-proof.txt", "content": "TARGET-WORKSPACE"})}}]}
                    finish = "tool_calls"
                else:
                    delta = {"content": "RESUMED-HISTORY-ANSWER"}
                    finish = "stop"
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Connection", "close")
                self.end_headers()
                chunk = {"choices": [{"index": 0, "delta": delta, "finish_reason": finish}]}
                self.wfile.write(("data: " + json.dumps(chunk) + "\n\ndata: [DONE]\n\n").encode())

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)

    def __enter__(self):
        """启动本地服务；返回实例。"""
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        return self

    def __exit__(self, *_args):
        """结束服务，参数为异常信息；返回无。"""
        self.server.shutdown()
        self.server.server_close()

    def config(self):
        """返回启用本地文件工具的合成配置；无参数。"""
        return {"active_provider": "fixture", "providers": [{"id": "fixture", "display_name": "Fixture",
            "base_url": f"http://127.0.0.1:{self.server.server_port}/v1", "api_key": "local-fixture",
            "models": ["fixture"], "default_model": "fixture"}], "tools": {"enabled": True},
            "session": {"auto_title_enabled": False}, "skills": {"enabled": False},
            "memory": {"enabled": False}, "permission": {"tui_mode": "yolo", "cli_mode": "yolo"}}
