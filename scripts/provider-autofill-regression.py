"""【设置密钥】【回归验证】隔离服务中验证自动填充不会覆盖供应商密钥。"""

import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request
import uuid


def request(origin, path, method="GET", body=None):
    """调用隔离接口；参数为地址、路径、方法和请求体，返回解析后的 JSON。"""
    payload = None if body is None else json.dumps(body).encode()
    query = urllib.request.Request(origin + path, data=payload, method=method,
                                   headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(query, timeout=10) as response:
        return json.load(response)


def browser(session, *args):
    """执行独立浏览器操作；参数为会话名和命令，返回结果，不输出页面敏感内容。"""
    result = subprocess.run(["agent-browser", "--session", session, "--json", *args],
                            capture_output=True, text=True, timeout=40, check=True)
    data = json.loads(result.stdout)
    assert data["success"], "浏览器操作失败"
    return data.get("data", {}).get("result")


def seed(origin):
    """创建两个虚构供应商；参数为隔离服务地址，返回无，不使用真实凭据。"""
    config = request(origin, "/api/config")["config"]
    config["providers"] = [
        {"id": f"fixture-{suffix}", "display_name": f"Fixture {suffix.upper()}",
         "base_url": origin + "/mock", "default_model": "fixture-model",
         "models": ["fixture-model"], "api_key_selected": "key-1",
         "api_keys": [{"id": "key-1", "api_key": f"fixture-original-{suffix}"}]}
        for suffix in ["a", "b"]
    ]
    config["active_provider"] = "fixture-a"
    config["provider_setup_complete"] = True
    request(origin, "/api/config", "PUT", config)


def assert_secret(origin, provider, expected):
    """核对虚构供应商持久化密钥；参数为地址、标识与预期，返回无，只报告断言结果。"""
    result = request(origin, "/api/config/provider-secret", "POST",
                     {"provider_id": provider, "key_id": "key-1"})
    assert result["api_key"] == expected, "供应商密钥被意外覆盖"


def verify(origin, session):
    """执行自动填充、保存和主动编辑流程；参数为服务和浏览器会话，返回验证摘要。"""
    seed(origin)
    browser(session, "open", origin + "/settings/providers/connection?item=fixture-a")
    browser(session, "wait", "--fn", "!!document.querySelector('.provider-api-key-value')")
    # 1. 【设置密钥】【自动填充】重放浏览器填充事件，旧代码会立刻污染草稿
    browser(session, "eval", """(() => {
      for (const input of document.querySelectorAll('.provider-api-key-value input')) {
        Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(input, 'fixture-autofilled-password');
        input.dispatchEvent(new InputEvent('input', {bubbles:true, inputType:'insertReplacementText', data:null}));
      }
    })()""")
    assert browser(session, "eval", "!document.querySelector('.settings-nav-dirty')"), "自动填充污染了草稿"
    assert browser(session, "eval", "!document.querySelector('.provider-api-key-value input')"), "尚未编辑就挂载了密码框"
    # 2. 【设置密钥】【保存无关字段】保存其他设置不能把登录密码写入密钥
    browser(session, "fill", 'input[aria-label="Key 1 note"]', "Updated note")
    browser(session, "find", "role", "button", "click", "--name", "Save", "--exact")
    browser(session, "wait", "--fn", "!document.querySelector('.settings-nav-dirty')")
    assert_secret(origin, "fixture-a", "fixture-original-a")
    assert_secret(origin, "fixture-b", "fixture-original-b")
    # 3. 【设置密钥】【仅查看】查看明文保持无输入框和无草稿修改
    browser(session, "click", '.provider-api-key-value button[aria-label="Show password"]')
    browser(session, "wait", "--fn", "document.querySelector('.ui-password-field-value')?.textContent==='fixture-original-a'")
    assert browser(session, "eval", "!document.querySelector('.settings-nav-dirty') && !document.querySelector('.provider-api-key-value input')")
    browser(session, "click", '.provider-api-key-value button[aria-label="Hide password"]')
    # 4. 【设置密钥】【主动替换】显式编辑才接收新值，保存后重新锁定
    browser(session, "click", '.provider-api-key-value button[aria-label="Edit secret"]')
    browser(session, "fill", '.provider-api-key-value input', "fixture-replacement-a")
    browser(session, "click", '.provider-api-key-value button[aria-label="Finish editing"]')
    browser(session, "find", "role", "button", "click", "--name", "Save", "--exact")
    browser(session, "wait", "--fn", "!document.querySelector('.settings-nav-dirty')")
    assert_secret(origin, "fixture-a", "fixture-replacement-a")
    # 5. 【设置密钥】【供应商隔离】切换相同 key-id 的供应商不会继承明文或编辑状态
    browser(session, "find", "role", "button", "click", "--name", "Fixture B fixture-model", "--exact")
    browser(session, "wait", "--fn", "location.search.includes('fixture-b')")
    assert browser(session, "eval", "!document.querySelector('.provider-api-key-value input') && !document.body.textContent.includes('fixture-replacement-a')")
    browser(session, "click", '.provider-api-key-value button[aria-label="Show password"]')
    browser(session, "wait", "--fn", "document.querySelector('.ui-password-field-value')?.textContent==='fixture-original-b'")
    browser(session, "find", "role", "button", "click", "--name", "Fixture A fixture-model", "--exact")
    browser(session, "wait", "--fn", "location.search.includes('fixture-a')")
    assert browser(session, "eval", "!document.body.textContent.includes('fixture-original-b')")
    # 6. 【设置密钥】【清除与新增】显式清除后重新填写仍然可用
    browser(session, "click", '.provider-api-key-value button[aria-label="Clear the saved value"]')
    browser(session, "find", "role", "button", "click", "--name", "Save", "--exact")
    browser(session, "wait", "--fn", "!document.querySelector('.settings-nav-dirty')")
    config = request(origin, "/api/config")["config"]
    current = next(item for item in config["providers"] if item["id"] == "fixture-a")
    assert all(not item.get("api_key") for item in current.get("api_keys", []))
    browser(session, "click", '.provider-api-key-value button[aria-label="Edit secret"]')
    browser(session, "fill", '.provider-api-key-value input', "fixture-restored-a")
    browser(session, "find", "role", "button", "click", "--name", "Save", "--exact")
    browser(session, "wait", "--fn", "!document.querySelector('.settings-nav-dirty')")
    assert browser(session, "eval", "!document.querySelector('.provider-api-key-value input')")
    assert_secret(origin, "fixture-a", "fixture-restored-a")
    assert_secret(origin, "fixture-b", "fixture-original-b")
    browser(session, "set", "viewport", "390", "844")
    assert browser(session, "eval", "document.documentElement.scrollWidth<=innerWidth")
    return {"autofill_preserved_key": True, "unrelated_save_preserved_key": True,
            "reveal_read_only": True, "manual_edit": True, "provider_isolation": True,
            "clear_and_reenter": True, "mobile_no_overflow": True}


def main(verify_case=verify):
    """启动隔离服务并清理进程；verify_case 为浏览器验证函数，失败返回非零状态。"""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/debug/sai"))
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if sys.platform != "linux":
        parser.error("This isolated regression requires Linux/XDG directories")
    session = "sai-credential-" + uuid.uuid4().hex[:8]
    with tempfile.TemporaryDirectory(prefix="sai-credential-regression-") as directory:
        root = Path(directory)
        workspace = root / "workspace"
        workspace.mkdir()
        environment = dict(os.environ)
        for kind in ["CONFIG", "DATA", "STATE", "CACHE"]:
            environment[f"XDG_{kind}_HOME"] = str(root / kind.lower())
        environment["SAI_LANG"] = "en_US"
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        origin = f"http://127.0.0.1:{port}"
        with (root / "server.log").open("w") as log:
            process = subprocess.Popen([str(args.binary.resolve()), "web", "--host", "127.0.0.1",
                                        "--port", str(port), "--allow-anonymous", "--no-open",
                                        "--workspace", str(workspace)], env=environment, cwd=workspace,
                                       stdout=log, stderr=log)
            try:
                deadline = time.monotonic() + 30
                while True:
                    try:
                        request(origin, "/api/config")
                        break
                    except OSError:
                        if process.poll() is not None or time.monotonic() >= deadline:
                            raise RuntimeError("隔离服务启动失败") from None
                        time.sleep(0.1)
                result = verify_case(origin, session)
                if args.output:
                    args.output.write_text(json.dumps(result, indent=2) + "\n")
                print(json.dumps(result))
            finally:
                subprocess.run(["agent-browser", "--session", session, "close"], capture_output=True, timeout=10)
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


if __name__ == "__main__":
    main()
