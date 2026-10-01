"""【供应商配置】【模型刷新回归】验证保存接入变更后立即使用新模型。"""

import json
import unittest

from mock_endpoints import MockEndpoints
from terminal import TerminalSession


class ProviderModelTests(unittest.TestCase):
    """通过真实配置表单和 /model 验证新增、修改与导入路径。"""

    def open_providers(self, terminal):
        """terminal 为终端会话；进入供应商列表，返回无。"""
        terminal.send(b"/config\r")
        terminal.wait_for(lambda: "SAI CONFIG" in terminal.text())
        terminal.send(b"2\r")
        terminal.wait_for(lambda: "PROVIDERS & MODELS" in terminal.text())

    def replace_field(self, terminal, value):
        """terminal 为终端，value 为新文本；替换当前字段并结束编辑，返回无。"""
        terminal.send(b"\r\x1b[F" + b"\x7f" * 160 + value.encode() + b"\r")
        terminal.pump(.1)

    def finish_configuration(self, terminal):
        """从供应商列表保存退出；terminal 为终端，返回无。"""
        terminal.send(b"q")
        terminal.wait_for(lambda: "SAI CONFIG" in terminal.text())
        terminal.send(b"8\r")
        terminal.wait_for(lambda: "Unsaved changes" in terminal.text())
        terminal.send(b"\r")
        terminal.wait_for(lambda: "configuration reloaded" in terminal.text())

    def save_and_choose(self, terminal, provider_id):
        """保存配置并在 /model 选择导入模型；参数为终端和供应商 ID，返回无。"""
        terminal.send(b"5\r")
        terminal.wait_for(lambda: "PROVIDERS & MODELS" in terminal.text())
        self.finish_configuration(terminal)
        terminal.send(b"/model\r")
        terminal.wait_for(lambda: "Type to filter" in terminal.text())
        terminal.send(b"fixture-image")
        terminal.pump(.3)
        self.assertIn("/ fixture-image", terminal.text(), "保存接入配置后 /model 缺少新模型")
        terminal.send(b"\r")
        terminal.wait_for(lambda: "Type to filter" not in terminal.text())
        config = json.loads((terminal.root / "config/sai/config.jsonc").read_text())
        self.assertEqual(config["active_provider"], provider_id)
        provider = next(item for item in config["providers"] if item["id"] == provider_id)
        self.assertEqual(provider["default_model"], "fixture-image")
        self.assertIn("fixture-image", provider["models"])

    def test_changed_connection_refreshes_models_on_save(self):
        """更换地址后直接保存供应商，应导入新目录且保留本地模型；返回无。"""
        with MockEndpoints() as server:
            config = server.config()
            config["providers"][0]["base_url"] = "http://127.0.0.1:1/v1"
            with TerminalSession(config=config, arguments=["--lang", "en-US"]) as terminal:
                self.open_providers(terminal)
                terminal.send(b"\r1\r")
                terminal.wait_for(lambda: "CONNECTION" in terminal.text())
                terminal.send(b"\x1b[B\x1b[B")
                self.replace_field(terminal, f"http://127.0.0.1:{server.server.server_port}/v1")
                terminal.send(b"s")
                terminal.wait_for(lambda: "EDIT PROVIDER" in terminal.text())
                self.save_and_choose(terminal, "test")
                config = json.loads((terminal.root / "config/sai/config.jsonc").read_text())
                self.assertIn("test", config["providers"][0]["models"])
                self.assertTrue(any(auth == "Bearer fixture-b" for _, auth, _ in server.requests))

    def test_new_provider_refreshes_models_on_save(self):
        """新增供应商后直接保存，应在同一会话的 /model 中可选；返回无。"""
        with MockEndpoints() as server, TerminalSession(arguments=["--lang", "en-US"]) as terminal:
            self.open_providers(terminal)
            terminal.send(b"a1\r")
            terminal.wait_for(lambda: "CONNECTION" in terminal.text())
            self.replace_field(terminal, "new-fixture")
            terminal.send(b"\x1b[B")
            self.replace_field(terminal, "New fixture")
            terminal.send(b"\x1b[B")
            self.replace_field(terminal, f"http://127.0.0.1:{server.server.server_port}/v1")
            terminal.send(b"\x1b[B\x1b[B")
            self.replace_field(terminal, "new-fixture-key")
            terminal.send(b"s")
            terminal.wait_for(lambda: "EDIT PROVIDER" in terminal.text())
            self.save_and_choose(terminal, "new-fixture")

    def test_failed_import_preserves_active_provider_and_manual_model_entry(self):
        """新增供应商导入失败后仍能保存并手动添加模型，原激活项保持可用；返回无。"""
        with TerminalSession(arguments=["--lang", "en-US"]) as terminal:
            self.open_providers(terminal)
            terminal.send(b"a1\r")
            terminal.wait_for(lambda: "CONNECTION" in terminal.text())
            self.replace_field(terminal, "offline-fixture")
            terminal.send(b"\x1b[B\x1b[B")
            self.replace_field(terminal, "http://127.0.0.1:1/v1")
            terminal.send(b"s")
            terminal.wait_for(lambda: "EDIT PROVIDER" in terminal.text())
            terminal.send(b"5\r")
            terminal.wait_for(lambda: "Could not import models" in terminal.text())
            terminal.send(b"\r")
            terminal.wait_for(lambda: "PROVIDERS & MODELS" in terminal.text())
            self.finish_configuration(terminal)
            config = json.loads((terminal.root / "config/sai/config.jsonc").read_text())
            self.assertEqual(config["active_provider"], "test")
            self.assertTrue(any(item["id"] == "offline-fixture" for item in config["providers"]))
            self.open_providers(terminal)
            # 1. 按列表位置选中离线供应商，再在模型列手动添加
            index = next(i for i, item in enumerate(config["providers"]) if item["id"] == "offline-fixture")
            terminal.send(b"\x1b[B" * index + b"\x1b[C\x1b[Ca")
            terminal.wait_for(lambda: "ADD CUSTOM MODEL" in terminal.text())
            self.replace_field(terminal, "manual-model")
            terminal.send(b"s")
            terminal.wait_for(lambda: "PROVIDERS & MODELS" in terminal.text())
            self.finish_configuration(terminal)
            terminal.send(b"/model\r")
            terminal.wait_for(lambda: "Type to filter" in terminal.text())
            terminal.send(b"manual-model")
            terminal.pump(.3)
            self.assertIn("/ manual-model", terminal.text())

    def test_explicit_import_is_available_without_saving_connection_again(self):
        """显式导入后保存并选择模型，沿用当前选中密钥；返回无。"""
        with MockEndpoints() as server, TerminalSession(config=server.config(), arguments=["--lang", "en-US"]) as terminal:
            self.open_providers(terminal)
            terminal.send(b"\r4\r")
            terminal.wait_for(lambda: "models imported" in terminal.text())
            terminal.send(b"\r")
            terminal.wait_for(lambda: "EDIT PROVIDER" in terminal.text())
            self.save_and_choose(terminal, "test")
