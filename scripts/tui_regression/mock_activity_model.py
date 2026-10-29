"""【终端】【执行链模型】暂停思考和真实工具，验证线性折叠链的阶段切换。"""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import threading


class ActivityModel:
    """用本地响应和隔离文件控制执行阶段，不访问外部服务。"""

    def __init__(self, tool_name="run_command", arguments=None):
        """参数 tool_name 为工具名称，arguments 为参数；初始化思考闸门，返回无。"""
        self.ready = threading.Event()
        self.advance = threading.Event()
        self.command = ""
        self.tool_name = tool_name
        self.arguments = arguments
        self.answer = "ANSWER-END"
        self.reasoning = "THINK-CURRENT\nDetailed current reasoning."
        self.reasoning_gate = None
        self.answer_gate = None
        self.answer_ready = threading.Event()
        self.argument_chunks = None
        self.tool_calls = None
        self.preparing = threading.Event()
        self.resume_preparation = threading.Event()
        model = self

        class Handler(BaseHTTPRequestHandler):
            """先产生思考与命令调用，再依据工具结果返回回答。"""

            def log_message(self, *_args):
                """参数为访问日志字段；忽略常规日志，返回无。"""

            def do_POST(self):
                """无参数；输出可暂停的兼容响应流，返回无。"""
                request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                done = any(message.get("role") == "tool" for message in request["messages"])
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Connection", "close")
                self.end_headers()
                try:
                    if done:
                        model.answer_ready.set()
                        if model.answer_gate is not None and not model.answer_gate.wait(20):
                            return
                        self.emit({"content": model.answer}, "stop")
                    else:
                        if model.reasoning_gate is not None and not model.reasoning_gate.wait(20):
                            return
                        self.emit({"reasoning_content": model.reasoning})
                        model.ready.set()
                        if not model.advance.wait(20):
                            return
                        if model.argument_chunks is not None:
                            self.emit({"tool_calls": [{"index": 0, "id": "activity-command",
                                "type": "function", "function": {"name": model.tool_name,
                                "arguments": model.argument_chunks[0]}}]})
                            model.preparing.set()
                            if not model.resume_preparation.wait(20):
                                return
                            self.emit({"tool_calls": [{"index": 0, "function": {
                                "arguments": model.argument_chunks[1]}}]}, "tool_calls")
                        elif model.tool_calls is not None:
                            self.emit({"tool_calls": model.tool_calls}, "tool_calls")
                        else:
                            self.emit({"tool_calls": [{"index": 0, "id": "activity-command",
                            "type": "function", "function": {"name": model.tool_name,
                            "arguments": json.dumps(model.arguments if model.arguments is not None
                                                    else {"command": model.command})}}]}, "tool_calls")
                    self.wfile.write(b"data: [DONE]\n\n")
                    self.wfile.flush()
                except (BrokenPipeError, ConnectionResetError):
                    pass

            def emit(self, delta, finish=None):
                """参数 delta 为响应增量，finish 为终止原因；发送 SSE，返回无。"""
                chunk = {"choices": [{"index": 0, "delta": delta, "finish_reason": finish}]}
                self.wfile.write(("data: " + json.dumps(chunk) + "\n\n").encode())
                self.wfile.flush()

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)

    def __enter__(self):
        """无参数；启动本地服务器，返回当前模型。"""
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        return self

    def __exit__(self, *_args):
        """参数为异常信息；解除暂停并关闭服务器，返回无。"""
        self.advance.set()
        if self.reasoning_gate is not None:
            self.reasoning_gate.set()
        self.resume_preparation.set()
        if self.answer_gate is not None:
            self.answer_gate.set()
        self.server.shutdown()
        self.server.server_close()

    def config(self):
        """无参数；返回全屏视图与隔离命令工具配置。"""
        return {"active_provider": "fixture", "providers": [{"id": "fixture",
            "display_name": "Fixture", "base_url": f"http://127.0.0.1:{self.server.server_port}/v1",
            "api_key": "local-fixture", "models": ["fixture"], "default_model": "fixture"}],
            "display": {"fullscreen": True, "reasoning": "full"},
            "session": {"auto_title_enabled": False}, "tools": {"enabled": True},
            "skills": {"enabled": False}, "memory": {"enabled": False},
            "permission": {"tui_mode": "yolo", "cli_mode": "yolo"}}
