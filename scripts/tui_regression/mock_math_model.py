"""【终端】【模型替身】流式输出行内公式、单行块级公式与 aligned 多行公式。"""

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import threading

# 与用户截图一致的公式写法：定界符与首尾公式同行
MATH_REPLY = (
    "行内公式：质能等价 $E=mc^2$，欧拉公式 $e^{ix}=\\cos x+i\\sin x$。INLINE-TAIL\n\n"
    "1. 欧拉恒等式\n"
    "$$ e^{i\\pi}+1=0 $$\n"
    "5. 麦克斯韦方程组\n"
    "$$\\begin{aligned}\n"
    "\\nabla \\cdot \\mathbf{E} &= \\frac{\\rho}{\\varepsilon_0} \\\\\n"
    "\\nabla \\cdot \\mathbf{B} &= 0\n"
    "\\end{aligned}$$\n\n"
    "MATH-DONE\n"
)


class MockMathModel:
    """返回固定公式内容的本地 OpenAI 兼容服务。"""

    def __init__(self):
        """创建本地服务器；无参数。"""

        class Handler(BaseHTTPRequestHandler):
            """把请求转换为一段公式回复。"""

            def log_message(self, *_):
                """忽略 HTTP 日志；返回无。"""

            def do_POST(self):
                """分段发送公式回复并结束；返回无。"""
                self.rfile.read(int(self.headers.get("Content-Length", 0)))
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Connection", "close")
                self.end_headers()
                try:
                    for line in MATH_REPLY.splitlines(keepends=True):
                        self.emit({"content": line})
                    self.emit({}, "stop")
                    self.wfile.write(b"data: [DONE]\n\n")
                    self.wfile.flush()
                except (BrokenPipeError, ConnectionResetError):
                    pass

            def emit(self, delta, finish=None):
                """发送 delta 增量与可选结束原因；返回无。"""
                data = {"id": "fixture", "object": "chat.completion.chunk", "choices": [
                    {"index": 0, "delta": delta, "finish_reason": finish}]}
                self.wfile.write(("data: " + json.dumps(data) + "\n\n").encode())
                self.wfile.flush()

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)

    def __enter__(self):
        """启动服务器，返回实例。"""
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        return self

    def __exit__(self, *_):
        """关闭服务器；返回无。"""
        self.server.shutdown()
        self.server.server_close()

    def config(self):
        """返回只连接本地服务器的合成配置。"""
        return {"active_provider": "fixture", "providers": [{"id": "fixture",
            "display_name": "Fixture", "base_url": f"http://127.0.0.1:{self.server.server_port}/v1",
            "api_key": "local-fixture", "models": ["fixture"], "default_model": "fixture"}],
            "tools": {"enabled": False}, "skills": {"enabled": False}}
