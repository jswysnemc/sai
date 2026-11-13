"""【首次配置】【终端回归】覆盖初始化引导、取消、保存与旧配置升级。"""

import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import os
import threading
import unittest

from terminal import TerminalSession


def fresh_install(root, _environment, _binary):
    """删除测试夹具配置以模拟全新安装；root 为隔离目录，其余参数为环境和程序，返回无。"""
    (root / "config/sai/config.jsonc").unlink()


def setup_visible(terminal):
    """terminal 为测试终端；返回是否已经显示首次配置菜单。"""
    return "FIRST USE" in terminal.text()


def configuration(terminal):
    """terminal 为测试终端；返回磁盘配置。"""
    return json.loads((terminal.root / "config/sai/config.jsonc").read_text())


class ProviderOnboardingTests(unittest.TestCase):
    """验证首轮客户端创建之前完成供应商配置。"""

    def test_fresh_install_can_confirm_builtin_provider(self):
        """全新安装明确确认免费供应商后进入聊天，完成状态可以持久化；返回无。"""
        with TerminalSession(prepare=fresh_install, ready=setup_visible, arguments=["--lang", "en-US"]) as terminal:
            self.assertFalse(configuration(terminal)["provider_setup_complete"])
            terminal.send(b"\r")
            terminal.wait_for(lambda: "SAVE TO START" in terminal.text())
            terminal.send(b"s")
            terminal.wait_for(lambda: "SELECT MODEL" in terminal.text(), timeout=15)
            terminal.send(b"\r")
            terminal.wait_for(lambda: configuration(terminal)["provider_setup_complete"])
            terminal.wait_for(lambda: "PROVIDER SETUP" not in terminal.text() and "auto" in terminal.text().lower())
            saved = configuration(terminal)
            self.assertEqual(saved["active_provider"], "opencode")
            self.assertEqual(saved["session"]["new_session_provider_id"], "opencode")
        # 1. 再次使用已完成的同一配置时直接进入聊天
        with TerminalSession(config=saved, arguments=["--lang", "en-US"]) as terminal:
            self.assertNotIn("FIRST USE", terminal.text())

    def test_cancel_preserves_pending_state_and_restores_terminal(self):
        """取消退出本次启动，保持未完成并恢复备用屏；返回无。"""
        with TerminalSession(prepare=fresh_install, ready=setup_visible, arguments=["--lang", "en-US"]) as terminal:
            terminal.send(b"\x1b")
            terminal.wait_for(lambda: b"\x1b[?1049l" in terminal.raw)
            terminal.wait_for(lambda: os.waitpid(terminal.pid, os.WNOHANG)[0] == terminal.pid)
            saved = configuration(terminal)
            self.assertFalse(saved["provider_setup_complete"])
        with TerminalSession(config=saved, ready=setup_visible, arguments=["--lang", "en-US"]) as terminal:
            self.assertIn("FIRST USE", terminal.text())

    def test_missing_model_can_be_filled_before_chat_client_is_created(self):
        """默认模型为空时先引导填写，密钥在表单中保持隐藏；返回无。"""
        config = {"provider_setup_complete": False, "active_provider": "fixture", "providers": [{
            "id": "fixture", "display_name": "Setup fixture", "base_url": "http://127.0.0.1:1/v1",
            "api_key": "onboarding-secret", "default_model": "", "models": []}]}
        with TerminalSession(config=config, ready=setup_visible, arguments=["--lang", "en-US"]) as terminal:
            terminal.send(b"\r")
            terminal.wait_for(lambda: "SAVE TO START" in terminal.text())
            self.assertNotIn("onboarding-secret", terminal.text())
            terminal.send(b"s")
            terminal.wait_for(lambda: "SELECT MODEL" in terminal.text())
            terminal.send(b"\r")
            terminal.wait_for(lambda: "CUSTOM MODEL" in terminal.text())
            terminal.send(b"\rsetup-model\rs")
            terminal.wait_for(lambda: configuration(terminal)["provider_setup_complete"])
            self.assertEqual(configuration(terminal)["session"]["new_session_model"], "setup-model")

    def test_legacy_configuration_does_not_show_onboarding(self):
        """没有引导字段的旧配置直接进入聊天；返回无。"""
        with TerminalSession(arguments=["--lang", "en-US"]) as terminal:
            self.assertNotIn("FIRST USE", terminal.text())
            self.assertIn("auto", terminal.text().lower())

    def test_discovers_models_and_saves_user_selection_without_inference(self):
        """无参数；返回无值，配置阶段查询模型列表，选择结果持久化且不调用推理接口。"""
        requests = []

        class Handler(BaseHTTPRequestHandler):
            """【首次配置】【目录夹具】记录本地请求并返回两个模型。"""

            def log_message(self, *_args):
                """参数为访问日志；返回无值，避免输出认证字段。"""

            def do_GET(self):
                """无参数；返回兼容 models 列表并记录路径。"""
                requests.append(("GET", self.path))
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(b'{"data":[{"id":"model-a"},{"id":"model-b"}]}')

            def do_POST(self):
                """无参数；记录不应出现的推理请求并拒绝。"""
                requests.append(("POST", self.path))
                self.send_response(400)
                self.end_headers()

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        config = {"provider_setup_complete": False, "active_provider":"fixture", "providers":[{
            "id":"fixture", "display_name":"Catalog fixture", "base_url":f"http://127.0.0.1:{server.server_port}/v1/chat/completions", "api_key":"fixture", "models":[], "default_model":""}]}
        try:
            with TerminalSession(config=config, ready=setup_visible, arguments=["--lang", "en-US"]) as terminal:
                terminal.send(b"\r")
                terminal.wait_for(lambda: "SAVE TO START" in terminal.text())
                terminal.send(b"s")
                terminal.wait_for(lambda: "SELECT MODEL" in terminal.text() and "model-a" in terminal.text())
                terminal.send(b"\x1b[B\r")
                terminal.wait_for(lambda: configuration(terminal)["provider_setup_complete"])
                saved = configuration(terminal)
                self.assertEqual(saved["session"]["new_session_model"], "model-b")
                self.assertIn(("GET", "/v1/models"), requests)
                self.assertFalse(any(method == "POST" for method, _ in requests))
        finally:
            server.shutdown()
            server.server_close()
