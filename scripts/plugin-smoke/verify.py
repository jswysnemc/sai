"""使用已构建的 sai 验收仓库外普通 Lua 包的完整管理路径。"""

import argparse
import hashlib
import json
import shutil
import sys
import tempfile
import time
from pathlib import Path

from fixture import Fixture


def verify(fixture: Fixture, example: Path):
    """【插件验收】【独立开发】接收隔离环境和示例目录，返回开发路径断言结果。"""
    # 1. 【插件验收】【模板改造】先验证原生模板，再把示例作为普通源码包复制到仓库外
    fixture.plugins("init", "project-notes", fixture.source)
    template = fixture.plugins("check", fixture.source)
    assert template["tools"][0]["name"] == "greet"
    for path in example.iterdir():
        if path.name == "sai-plugin.json" or path.suffix == ".lua":
            shutil.copyfile(path, fixture.source / path.name)
    inspection = fixture.plugins("check", fixture.source)
    assert [tool["name"] for tool in inspection["tools"]] == ["inspect"]
    assert len(inspection["commands"]) == 5
    assert inspection["events"] == ["tool_result"]
    fixture.plugins("install", fixture.source)
    installed = fixture.info()
    assert installed["enabled"] is False
    assert "installed" in json.dumps(installed["source"]).lower()
    fixture.command("inspect", succeeds=False)

    # 2. 【插件验收】【最小授权】读取和持久存储分别授予，写入仍受计划模式约束
    note = fixture.project / "notes/README.md"
    note.parent.mkdir()
    contents = "# 项目笔记\n\n独立插件通过公开接口读取本文件。\n"
    note.write_text(contents, encoding="utf-8")
    settings = {"path": "notes/README.md", "preview_chars": 15}
    fixture.configure(settings)
    fixture.plugins("enable", "project-notes")
    fixture.command("inspect", succeeds=False)
    fixture.plugins("enable", "project-notes", "--allow-read-path", "notes")
    inspected = fixture.command("inspect")
    assert inspected["sha256"] == hashlib.sha256(contents.encode()).hexdigest()
    assert inspected["bytes"] == len(contents.encode())
    assert Path(inspected["path"]) == note.resolve()
    fixture.command("latest", succeeds=False)
    fixture.plugins("enable", "project-notes", "--allow-plugin-storage")
    assert fixture.command("latest") == {"record": None}
    fixture.command("remember", succeeds=False)
    assert fixture.command("latest") == {"record": None}
    remembered = fixture.command("remember", mode="yolo")
    assert remembered["sha256"] == inspected["sha256"]
    assert fixture.command("latest")["record"] == remembered
    assert "settings" not in fixture.info()

    # 3. 【插件验收】【错误定位】非法设置不覆盖旧配置，截断和越界读取不能保存伪完整记录
    previous = fixture.config.read_bytes()
    error = fixture.configure({"preview_chars": False}, succeeds=False)
    assert "preview_chars" in error
    assert fixture.config.read_bytes() == previous
    note.write_bytes(b"x" * 65537)
    assert "exceeds 65536 bytes" in fixture.command("inspect", succeeds=False)
    fixture.command("remember", mode="yolo", succeeds=False)
    assert fixture.command("latest")["record"] == remembered
    note.write_bytes(b"\xff")
    fixture.command("inspect", succeeds=False)
    note.write_text(contents, encoding="utf-8")
    (fixture.project / "outside.txt").write_text("outside notes", encoding="utf-8")
    fixture.configure({"path": "outside.txt"})
    fixture.command("inspect", succeeds=False)
    fixture.configure(settings)

    # 4. 【插件验收】【替换更新】源码目录修改不影响安装快照，新能力不会自动取得授权
    manifest_path = fixture.source / "sai-plugin.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["version"] = "0.2.0"
    manifest["capabilities"]["system"]["environment"] = ["LANG"]
    manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
    inspect_path = fixture.source / "inspect.lua"
    source = inspect_path.read_text(encoding="utf-8")
    inspect_path.write_text(source.replace("        path = sai.fs.realpath(path),",
        '        source_version = "0.2.0",\n        path = sai.fs.realpath(path),'), encoding="utf-8")
    assert "source_version" not in fixture.command("inspect")
    fixture.plugins("install", fixture.source, succeeds=False)
    fixture.plugins("check", fixture.source)
    fixture.plugins("install", fixture.source, "--replace")
    updated = fixture.info()
    assert updated["version"] == "0.2.0"
    system = updated["effective_grants"]["system"]
    assert system["read_paths"] == ["notes"] and system["plugin_storage"] is True
    assert not system.get("environment")
    assert json.loads(fixture.config.read_text())["plugins"]["project-notes"]["settings"] == settings
    assert fixture.command("inspect")["source_version"] == "0.2.0"
    assert fixture.command("latest")["record"] == remembered

    # 5. 【插件验收】【撤权卸载】普通启用不恢复撤销权限，显式清理数据后禁用并移除源码
    fixture.plugins("enable", "project-notes", "--no-file-read")
    fixture.command("inspect", succeeds=False)
    assert fixture.command("latest")["record"] == remembered
    fixture.plugins("enable", "project-notes", "--no-plugin-storage")
    fixture.command("latest", succeeds=False)
    fixture.plugins("enable", "project-notes")
    fixture.command("latest", succeeds=False)
    fixture.plugins("enable", "project-notes", "--allow-plugin-storage")
    assert fixture.command("latest")["record"] == remembered
    fixture.command("forget", mode="yolo")
    assert fixture.command("latest") == {"record": None}
    fixture.plugins("disable", "project-notes")
    assert fixture.info()["enabled"] is False
    fixture.command("inspect", succeeds=False)
    assert all(item["plugin"] != "project-notes" for item in fixture.plugins("commands"))
    fixture.plugins("remove", "project-notes")
    fixture.plugins("info", "project-notes", succeeds=False)
    setting = json.loads(fixture.config.read_text())["plugins"]["project-notes"]
    assert setting["enabled"] is False
    assert not setting["grants"].get("system")
    assert not (fixture.root / "config/sai/plugins/project-notes").exists()
    assert not (fixture.root / "config/sai/config.jsonc").exists()


def main():
    """【插件验收】【执行入口】解析程序及报告路径，返回成功状态并保留命令与耗时证据。"""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    arguments = parser.parse_args()
    if sys.platform != "linux":
        parser.error("This fixture uses Linux XDG isolation; other platforms are not supported.")
    binary = arguments.binary.resolve(strict=True)
    report = arguments.report.resolve()
    repository = Path(__file__).resolve().parents[2]
    example = repository / "examples/lua-plugins/project-notes"
    started = time.monotonic()
    evidence = {"binary": str(binary), "passed": False}
    with binary.open("rb") as handle:
        evidence["binary_sha256"] = hashlib.file_digest(handle, "sha256").hexdigest()
    with tempfile.TemporaryDirectory(prefix="sai-lua-smoke-") as directory:
        fixture = Fixture(binary, Path(directory))
        evidence["working_directory"] = str(fixture.project)
        evidence["calls"] = fixture.calls
        try:
            verify(fixture, example)
            evidence["passed"] = True
        finally:
            evidence["seconds"] = round(time.monotonic() - started, 3)
            report.parent.mkdir(parents=True, exist_ok=True)
            report.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"Passed {len(fixture.calls)} CLI calls in {evidence['seconds']}s; report: {report}")


if __name__ == "__main__":
    main()
