"""【模型接入】【测试服务】提供本地 Jev、模型目录和图片响应。"""

import base64
import http.server
import json
import struct
import threading
import zlib


def image_base64():
    """生成有效单像素 PNG；无参数，返回 Base64 字符串。"""
    def chunk(kind, content):
        """kind 为块类型，content 为数据；返回带校验和的 PNG 块。"""
        body = kind + content
        return struct.pack("!I", len(content)) + body + struct.pack("!I", zlib.crc32(body))
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack("!IIBBBBB", 1, 1, 8, 2, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(b"\0\xff\0\0"))
    png += chunk(b"IEND", b"")
    return base64.b64encode(png).decode()


class MockEndpoints:
    """记录本地请求，禁止测试访问真实模型服务。"""

    def __init__(self):
        """创建测试服务；无参数，返回无。"""
        self.requests = []
        owner = self

        class Handler(http.server.BaseHTTPRequestHandler):
            """返回合成模型响应。"""

            def respond(self, value):
                """value 为响应对象；写出 JSON，返回无。"""
                body = json.dumps(value).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def do_GET(self):
                """记录目录请求并返回模型；返回无。"""
                owner.requests.append((self.path, self.headers.get("Authorization"), None))
                self.respond({"data": [{"id": "fixture-image"}]})

            def do_POST(self):
                """记录模型测试请求，返回 Jev 答案或图片；返回无。"""
                body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                owner.requests.append((self.path, self.headers.get("Authorization"), body))
                if self.path == "/systemone":
                    self.respond({"model": "jev-fixture", "answers": {"probe": {"type": "noul", "noul": .99}}})
                else:
                    self.respond({"data": [{"b64_json": image_base64()}]})

            def log_message(self, *_args):
                """忽略测试访问日志，参数为日志内容；返回无。"""

        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    def __enter__(self):
        """启动服务；返回实例。"""
        self.thread.start()
        return self

    def __exit__(self, *_args):
        """关闭服务，参数为异常信息；返回无。"""
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()

    def config(self):
        """返回仅指向本地服务的配置；无参数。"""
        base = f"http://127.0.0.1:{self.server.server_port}"
        keys = [{"id": "a", "api_key": "fixture-a"}, {"id": "b", "api_key": "fixture-b"}]
        return {"active_provider": "test", "providers": [{
            "id": "test", "display_name": "Test", "base_url": base + "/v1",
            "api_key": "unused", "api_keys": keys, "api_key_selected": "b",
            "models": ["test"], "default_model": "test"}], "model_endpoints": [
            {"id": "jev", "kind": "jev", "name": "Jev fixture", "model": "jev-latest",
             "endpoint": base + "/systemone", "api_keys": keys, "api_key_selected": "b"},
            {"id": "image", "kind": "image_generation", "name": "Image fixture", "model": "fixture-image",
             "endpoint": base + "/v1/images/generations", "api_keys": keys, "api_key_selected": "b"}]}
