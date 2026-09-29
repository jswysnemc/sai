"""【模型接入】【终端回归】验证真实配置会话中的创建、保存、取消和密钥保留。"""

import json
import unittest

from terminal import TerminalSession
from mock_endpoints import MockEndpoints


def wait_label(terminal, english, chinese):
    """terminal 为终端，另两参为双语标签；等待画面就绪，返回无。"""
    terminal.wait_for(lambda: english in terminal.text() or chinese in terminal.text())


def open_endpoints(terminal, kind):
    """terminal 为终端，kind 为类型序号；进入接入列表，返回无。"""
    terminal.send(b"/config\r")
    terminal.wait_for(lambda: "SAI CONFIG" in terminal.text())
    terminal.send(b"7\r")
    wait_label(terminal, "MODEL CONNECTIONS", "模型接入")
    terminal.send(str(kind).encode() + b"\r")
    wait_label(terminal, "Add connection", "新增接入")


def leave_and_save(terminal):
    """terminal 为接入列表；返回主菜单并保存，返回无。"""
    terminal.send(b"q")
    wait_label(terminal, "MODEL CONNECTIONS", "模型接入")
    terminal.send(b"q")
    terminal.wait_for(lambda: "SAI CONFIG" in terminal.text())
    terminal.send(b"8\r")
    wait_label(terminal, "Unsaved changes", "未保存的更改")
    terminal.send(b"\r")
    terminal.wait_for(lambda: "auto" in terminal.text() and "SAI CONFIG" not in terminal.text())


class EndpointConfigurationTests(unittest.TestCase):
    """使用隔离配置验证 Jev 与生图接入全流程。"""

    def test_jev_image_and_catalog_use_selected_credentials(self):
        """通过界面执行两类测试与目录导入，并核对请求认证和内容；返回无。"""
        with MockEndpoints() as server, TerminalSession(config=server.config()) as terminal:
            open_endpoints(terminal, 1)
            terminal.send(b"\r")
            wait_label(terminal, "EDIT CONNECTION", "编辑接入")
            terminal.send(b"4\r")
            wait_label(terminal, "Jev connection OK", "Jev 连接正常")
            terminal.send(b"\r")
            wait_label(terminal, "EDIT CONNECTION", "编辑接入")
            terminal.send(b"q")
            wait_label(terminal, "Add connection", "新增接入")
            terminal.send(b"q")
            wait_label(terminal, "MODEL CONNECTIONS", "模型接入")
            terminal.send(b"2\r")
            wait_label(terminal, "IMAGE MODELS", "生图模型")
            terminal.send(b"\r")
            wait_label(terminal, "EDIT CONNECTION", "编辑接入")
            terminal.send(b"3\r")
            wait_label(terminal, "Models imported", "已导入模型")
            terminal.send(b"\r")
            wait_label(terminal, "EDIT CONNECTION", "编辑接入")
            terminal.send(b"4\r")
            wait_label(terminal, "Image endpoint returned a valid image", "生图接入返回了有效图片")
            terminal.send(b"\r")
            wait_label(terminal, "EDIT CONNECTION", "编辑接入")
            terminal.send(b"5\r")
            wait_label(terminal, "Add connection", "新增接入")
            leave_and_save(terminal)
            saved = json.loads((terminal.root / "config/sai/config.jsonc").read_text())
            image = next(item for item in saved["model_endpoints"] if item["kind"] == "image_generation")
            self.assertEqual(image["models"], ["fixture-image"])
            self.assertEqual([path for path, _, _ in server.requests], ["/systemone", "/v1/models", "/v1/images/generations"])
            self.assertTrue(all(key == "Bearer fixture-b" for _, key, _ in server.requests))
            self.assertEqual(server.requests[-1][2]["model"], "fixture-image")

    def test_create_jev_and_image_then_reload(self):
        """创建两类接入，保存并再次进入设置；返回无。"""
        with TerminalSession() as terminal:
            open_endpoints(terminal, 1)
            terminal.send(b"\r")
            wait_label(terminal, "EDIT CONNECTION", "编辑接入")
            terminal.send(b"5\r")
            wait_label(terminal, "Add connection", "新增接入")
            terminal.send(b" ")
            terminal.pump(.1)
            terminal.send(b"q")
            wait_label(terminal, "MODEL CONNECTIONS", "模型接入")
            terminal.send(b"2\r")
            wait_label(terminal, "IMAGE MODELS", "生图模型")
            terminal.send(b"\r")
            wait_label(terminal, "EDIT CONNECTION", "编辑接入")
            terminal.send(b"1\r")
            wait_label(terminal, "CONNECTION AND MODEL", "连接与模型")
            terminal.send(b"\r-fixture\rs")
            wait_label(terminal, "EDIT CONNECTION", "编辑接入")
            terminal.send(b"5\r")
            wait_label(terminal, "Add connection", "新增接入")
            leave_and_save(terminal)
            saved = json.loads((terminal.root / "config/sai/config.jsonc").read_text())
            self.assertEqual({item["kind"] for item in saved["model_endpoints"]}, {"jev", "image_generation"})
            self.assertEqual(saved["jev"]["endpoint_id"], "jev-1")
            image = next(item for item in saved["model_endpoints"] if item["kind"] == "image_generation")
            self.assertEqual(image["name"], "Image generation-fixture")
            open_endpoints(terminal, 2)
            terminal.wait_for(lambda: "Image generation-fixture" in terminal.text())

    def test_editing_jev_preserves_multiple_keys_and_cancel_discards(self):
        """编辑名称不会改写密钥；取消整个编辑器丢弃分区修改；返回无。"""
        endpoint = {
            "id": "jev-fixture", "kind": "jev", "name": "Jev fixture",
            "endpoint": "http://127.0.0.1:1/decide", "model": "jev-latest",
            "api_key": "single-fixture", "api_keys": [
                {"id": "a", "api_key": "fixture-a", "label": "first"},
                {"id": "b", "api_key": "fixture-b", "label": "second"}],
            "api_key_selected": "b", "api_key_balance": False,
            "models": ["jev-latest", "jev-fixture-model"]}
        config = {"active_provider": "test", "providers": [{
            "id": "test", "display_name": "Test", "base_url": "http://127.0.0.1:1/v1",
            "api_key": "local-fixture", "models": ["test"], "default_model": "test"}],
            "model_endpoints": [endpoint], "jev": {"endpoint_id": endpoint["id"]}}
        with TerminalSession(config=config) as terminal:
            open_endpoints(terminal, 1)
            for suffix, save in [(b"-discarded", False), (b"-saved", True)]:
                terminal.send(b"\r")
                wait_label(terminal, "EDIT CONNECTION", "编辑接入")
                terminal.send(b"1\r")
                wait_label(terminal, "CONNECTION AND MODEL", "连接与模型")
                terminal.send(b"\r" + suffix + b"\rs")
                wait_label(terminal, "EDIT CONNECTION", "编辑接入")
                terminal.send(b"5\r" if save else b"q")
                wait_label(terminal, "Add connection", "新增接入")
                self.assertNotIn("fixture-a", terminal.text())
                self.assertNotIn("fixture-b", terminal.text())
            leave_and_save(terminal)
            saved = json.loads((terminal.root / "config/sai/config.jsonc").read_text())["model_endpoints"][0]
            self.assertEqual(saved["name"], "Jev fixture-saved")
            for key in ["api_key", "api_keys", "api_key_selected", "models"]:
                self.assertEqual(saved[key], endpoint[key])
