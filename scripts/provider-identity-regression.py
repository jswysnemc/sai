"""【供应商设置】【身份回归】验证真实浏览器中新增、改名、改 ID 和保存。"""

from importlib import import_module

fixture = import_module("provider-autofill-regression")
browser = fixture.browser
request = fixture.request


def field(session, name, value):
    """通过关联标签填写字段；参数为会话、标签与值，返回无。"""
    browser(session, "find", "label", name, "fill", value, "--exact")


def assert_selected(session, name, provider_id):
    """验证表单及 URL 仍指向当前草稿；参数为会话、名称和 ID，返回无。"""
    state = browser(session, "eval", """({
      name: [...document.querySelectorAll('h2')].map(node => node.textContent),
      id: new URLSearchParams(location.search).get('item')
    })""")
    assert name in state["name"] and state["id"] == provider_id, f"改名后切换到其他供应商: {state}"


def save(session):
    """提交当前草稿并等待保存完成；session 为浏览器会话，返回无。"""
    browser(session, "find", "role", "button", "click", "--name", "Save", "--exact")
    browser(session, "wait", "--fn", "!document.querySelector('.settings-nav-dirty')")


def verify(origin, session):
    """验证新增和已有供应商改名、重命名后密钥隔离；参数为服务和会话，返回结果。"""
    fixture.seed(origin)
    config = request(origin, "/api/config")["config"]
    config["session"]["new_session_provider_id"] = "fixture-b"
    config["session"]["new_session_model"] = "fixture-model"
    config["context"]["compaction_provider_id"] = "fixture-b"
    config["context"]["compaction_model"] = "fixture-model"
    request(origin, "/api/config", "PUT", config)
    browser(session, "open", origin + "/settings/providers/connection?item=fixture-a")
    browser(session, "wait", "--fn", "!!document.querySelector('.provider-api-key-value')")
    browser(session, "find", "role", "button", "click", "--name", "Add provider", "--exact")
    field(session, "Display name", "Renamed fixture")
    assert_selected(session, "Renamed fixture", "Renamed fixture")
    # 1. 【供应商设置】【连续改名】模拟逐字输入，验证每次更新都保持当前对象
    browser(session, "find", "label", "Display name", "click", "--exact")
    browser(session, "press", "End")
    for char in " extra":
        browser(session, "press", "Space" if char == " " else char)
    assert_selected(session, "Renamed fixture extra", "Renamed fixture extra")
    field(session, "Provider ID", "new-fixture")
    browser(session, "press", "Tab")
    assert_selected(session, "Renamed fixture extra", "new-fixture")
    field(session, "Display name", "Independent name")
    assert_selected(session, "Independent name", "new-fixture")
    save(session)
    config = request(origin, "/api/config")["config"]
    assert next(item for item in config["providers"] if item["id"] == "fixture-a")["display_name"] == "Fixture A"
    assert next(item for item in config["providers"] if item["id"] == "new-fixture")["display_name"] == "Independent name"
    fixture.assert_secret(origin, "fixture-a", "fixture-original-a")
    # 2. 【供应商设置】【已有密钥】改 ID 后保存与查看仍使用原对象，其他供应商保持独立
    browser(session, "find", "role", "button", "click", "--name", "Fixture B fixture-model", "--exact")
    field(session, "Provider ID", "renamed-b")
    browser(session, "press", "Tab")
    assert_selected(session, "Fixture B", "renamed-b")
    browser(session, "click", '.provider-api-key-value button[aria-label="Show password"]')
    browser(session, "wait", "--fn", "document.querySelector('.ui-password-field-value')?.textContent==='fixture-original-b'")
    save(session)
    fixture.assert_secret(origin, "renamed-b", "fixture-original-b")
    fixture.assert_secret(origin, "fixture-a", "fixture-original-a")
    config = request(origin, "/api/config")["config"]
    assert config["session"]["new_session_provider_id"] == "renamed-b"
    assert config["context"]["compaction_provider_id"] == "renamed-b"
    browser(session, "reload")
    browser(session, "wait", "--fn", "document.querySelector('h2')?.textContent==='Fixture B'")
    assert_selected(session, "Fixture B", "renamed-b")
    # 3. 【供应商设置】【无效标识】冲突和空值不改变当前供应商
    field(session, "Provider ID", "fixture-a")
    browser(session, "press", "Tab")
    assert_selected(session, "Fixture B", "renamed-b")
    assert browser(session, "eval", "document.body.textContent.includes('Provider ID is already used')")
    field(session, "Provider ID", "")
    browser(session, "press", "Tab")
    assert_selected(session, "Fixture B", "renamed-b")
    assert browser(session, "eval", "document.body.textContent.includes('Provider ID cannot be empty')")
    # 4. 【供应商设置】【草稿放弃】改名后放弃恢复有效选择，保留其他查询参数
    field(session, "Provider ID", "discarded-id")
    browser(session, "press", "Tab")
    browser(session, "find", "role", "button", "click", "--name", "Discard", "--exact")
    browser(session, "find", "role", "button", "click", "--name", "Discard changes", "--exact")
    browser(session, "wait", "--fn", "!document.querySelector('.settings-nav-dirty')")
    assert browser(session, "eval", "!document.body.textContent.includes('discarded-id')")
    browser(session, "find", "role", "button", "click", "--name", "Fixture A fixture-model", "--exact")
    # 5. 【供应商设置】【保存期间编辑】延迟一次真实保存，再改 ID，后续保存仍能恢复密钥
    field(session, "Provider ID", "inflight-a")
    browser(session, "press", "Tab")
    browser(session, "eval", """(() => {
      const originalFetch = window.fetch;
      window.fetch = async (...args) => {
        if (String(args[0]).endsWith('/api/config') && args[1]?.method === 'PUT') {
          window.fetch = originalFetch;
          await new Promise(resolve => { window.releaseIdentitySave = resolve; });
        }
        return originalFetch(...args);
      };
    })()""")
    browser(session, "find", "role", "button", "click", "--name", "Save", "--exact")
    browser(session, "wait", "--fn", "typeof window.releaseIdentitySave==='function'")
    field(session, "Provider ID", "active-renamed")
    browser(session, "press", "Tab")
    browser(session, "eval", "window.releaseIdentitySave()")
    browser(session, "wait", "--fn", "document.querySelector('.settings-save-button')?.disabled===false")
    assert_selected(session, "Fixture A", "active-renamed")
    save(session)
    assert request(origin, "/api/config")["config"]["active_provider"] == "active-renamed"
    fixture.assert_secret(origin, "active-renamed", "fixture-original-a")
    browser(session, "set", "viewport", "390", "844")
    assert browser(session, "eval", "document.documentElement.scrollWidth<=innerWidth")
    return {"new_provider_rename": True, "manual_id": True, "saved_secret_preserved": True,
            "provider_isolation": True, "invalid_id": True, "discard": True,
            "edit_during_save": True, "mobile": True}


if __name__ == "__main__":
    fixture.main(verify)
