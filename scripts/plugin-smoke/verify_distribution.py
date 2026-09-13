"""使用已构建的 sai 验收源码归档、替换更新及清单保存边界。"""

import argparse
import copy
import hashlib
import json
import os
import shutil
import sys
import tarfile
import tempfile
import time
from pathlib import Path, PurePosixPath

from fixture import Fixture


MANIFEST_LIMIT = 64 * 1024


def unpack_verified(result, source: Path, extraction: Path):
    """【插件分发验收】【归档核对】接收打包结果、源码及解压目录，返回校验后的包目录。"""
    path = Path(result["path"])
    data = path.read_bytes()
    assert result["bytes"] == len(data)
    assert result["sha256"] == hashlib.sha256(data).hexdigest()
    assert data[4:8] == bytes(4)
    assert data[9] == 255
    sources = {
        file.relative_to(source).as_posix(): file.read_bytes()
        for file in source.rglob("*.lua")
        if ".git" not in file.relative_to(source).parts
    }
    expected = [f"{result['id']}/sai-plugin.json"]
    expected.extend(f"{result['id']}/{name}" for name in sorted(sources))
    with tarfile.open(path, "r:gz") as archive:
        members = archive.getmembers()
        assert [member.name for member in members] == expected == result["files"]
        for member in members:
            relative = PurePosixPath(member.name)
            assert not relative.is_absolute() and ".." not in relative.parts
            assert member.isfile() and not member.linkname
            assert (member.mode, member.uid, member.gid, member.mtime) == (0o644, 0, 0, 0)
            content = archive.extractfile(member).read()
            name = relative.relative_to(result["id"]).as_posix()
            if name == "sai-plugin.json":
                manifest = json.loads(content)
                assert len(content) <= MANIFEST_LIMIT
                for key in ("id", "version", "api_version"):
                    assert manifest[key] == result[key]
            else:
                assert content == sources[name]
        archive.extractall(extraction, filter="data")
    return extraction / result["id"]


def verify_distribution(fixture: Fixture, example: Path):
    """【插件分发验收】【发布更新】接收隔离环境和示例，返回真实归档与更新结果。"""
    # 1. 【插件分发验收】【最小内容】在仓库外加入本地文件，确认归档只包含运行源码
    shutil.copytree(example, fixture.source)
    (fixture.source / ".env").write_text("TOKEN=fixture-only\n", encoding="utf-8")
    (fixture.source / "settings.json").write_text('{"local":true}\n', encoding="utf-8")
    (fixture.source / ".git").mkdir()
    (fixture.source / ".git/ignored.lua").write_text("not valid Lua\n", encoding="utf-8")
    first = fixture.plugins("pack", fixture.source)
    assert Path(first["path"]) == fixture.project / "project-notes-0.1.0.tar.gz"
    assert not fixture.config.exists()
    original_bytes = Path(first["path"]).read_bytes()
    extracted = unpack_verified(first, fixture.source, fixture.root / "first")
    checked = fixture.plugins("check", extracted)
    assert [tool["name"] for tool in checked["tools"]] == ["inspect"]
    assert len(checked["commands"]) == 5 and checked["events"] == ["tool_result"]

    # 2. 【插件分发验收】【确定输出】源码位置、文件时间和输出名称不改变压缩内容
    relocated = fixture.root / "relocated"
    shutil.copytree(fixture.source, relocated)
    os.utime(relocated / "init.lua", (1, 1))
    repeated = fixture.plugins("pack", relocated, "-o", "releases/repeated.tgz")
    assert Path(repeated["path"]) == fixture.project / "releases/repeated.tgz"
    assert Path(repeated["path"]).read_bytes() == original_bytes
    assert repeated["sha256"] == first["sha256"]
    assert "already exists" in fixture.plugins("pack", fixture.source, succeeds=False)
    assert Path(first["path"]).read_bytes() == original_bytes

    # 3. 【插件分发验收】【安装执行】从归档恢复的普通包经过配置和最小授权后执行
    installed = fixture.plugins("install", extracted)
    assert fixture.info()["enabled"] is False
    note = fixture.project / "notes/release.md"
    note.parent.mkdir()
    contents = "# Release note\nPackaged command works.\n"
    note.write_text(contents, encoding="utf-8")
    settings = {"path": "notes/release.md", "preview_chars": 24}
    fixture.configure(settings)
    fixture.plugins("enable", "project-notes", "--allow-read-path", "notes",
                    "--allow-plugin-storage")
    inspected = fixture.command("inspect")
    assert Path(inspected["path"]) == note
    assert inspected["sha256"] == hashlib.sha256(contents.encode()).hexdigest()
    assert inspected["bytes"] == len(contents.encode())
    remembered = fixture.command("remember", mode="yolo")
    original_grants = fixture.info()["effective_grants"]

    # 4. 【插件分发验收】【版本替换】发布新增声明的版本，保留设置、记录和既有授权
    manifest_path = fixture.source / "sai-plugin.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["version"] = "0.2.0"
    manifest["capabilities"]["system"]["environment"] = ["LANG"]
    manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
    module_path = fixture.source / "inspect.lua"
    module = module_path.read_text(encoding="utf-8")
    insertion = "        path = sai.fs.realpath(path),"
    assert module.count(insertion) == 1
    module_path.write_text(module.replace(insertion,
        '        source_version = "0.2.0",\n' + insertion), encoding="utf-8")
    release = fixture.plugins("pack", fixture.source, "--output", "releases/project-notes-0.2.0.tar.gz")
    updated_source = unpack_verified(release, fixture.source, fixture.root / "updated")
    assert "source_version" not in fixture.command("inspect")
    assert "use --replace" in fixture.plugins("install", updated_source, succeeds=False)
    fixture.plugins("check", updated_source)
    fixture.plugins("install", updated_source, "--replace")
    updated = fixture.info()
    assert updated["version"] == "0.2.0" and updated["enabled"] is True
    assert updated["effective_grants"] == original_grants
    saved = json.loads(fixture.config.read_text())["plugins"]["project-notes"]
    assert saved["settings"] == settings
    assert fixture.command("inspect")["source_version"] == "0.2.0"
    assert fixture.command("latest")["record"] == remembered
    assert Path(first["path"]).read_bytes() == original_bytes

    # 5. 【插件分发验收】【撤权移除】安装自归档的包继续遵守既有管理与授权边界
    fixture.plugins("enable", "project-notes", "--no-file-read")
    fixture.command("inspect", succeeds=False)
    fixture.command("forget", mode="yolo")
    assert fixture.command("latest") == {"record": None}
    fixture.plugins("disable", "project-notes")
    fixture.command("latest", succeeds=False)
    fixture.plugins("remove", "project-notes")
    assert not Path(installed["path"]).exists()
    assert not (fixture.root / "config/sai/config.jsonc").exists()
    return {"archives": [first, repeated, release], "updated_settings": settings,
            "preserved_grants": original_grants, "record_preserved": True}


def boundary_manifest(base, *, defaults_overflow=False):
    """【插件分发验收】【边界清单】接收完整清单及超限模式，返回原始 JSON 与规范字段。"""
    normalized = copy.deepcopy(base)
    for width in range(3900, 4070):
        normalized["capabilities"] = {
            "http": [],
            "system": {"read_paths": sorted(f"notes/{index}/" + "a" * width for index in range(16))}
        }
        source = copy.deepcopy(normalized)
        del source["capabilities"]["http"]
        if defaults_overflow:
            del source["limits"]
        encoded = json.dumps(source, ensure_ascii=False, separators=(",", ":")).encode()
        compact = len(json.dumps(normalized, ensure_ascii=False, separators=(",", ":")).encode()) + 1
        pretty = len(json.dumps(normalized, ensure_ascii=False, indent=2).encode()) + 1
        matches = compact > MANIFEST_LIMIT if defaults_overflow else compact <= MANIFEST_LIMIT < pretty
        if len(encoded) <= MANIFEST_LIMIT and matches:
            return encoded, normalized
    raise AssertionError("No valid manifest boundary fixture found")


def verify_manifest_boundary(fixture: Fixture):
    """【插件分发验收】【保存边界】接收隔离环境，返回清单往返及拒绝超限的实际字节数。"""
    # 1. 【插件分发验收】【格式膨胀】源码有效且缩进将超限时，安装和归档使用紧凑 JSON
    source = fixture.root / "manifest-boundary"
    fixture.plugins("init", "manifest-boundary", source)
    manifest_path = source / "sai-plugin.json"
    base = json.loads(manifest_path.read_text(encoding="utf-8"))
    raw, normalized = boundary_manifest(base)
    manifest_path.write_bytes(raw)
    assert fixture.plugins("check", source)["manifest"] == normalized
    packed = fixture.plugins("pack", source, "-o", "releases/manifest-boundary.tar.gz")
    extracted = unpack_verified(packed, source, fixture.root / "boundary")
    assert fixture.plugins("check", extracted)["manifest"] == normalized
    installed = Path(fixture.plugins("install", source)["path"])
    saved = (installed / "sai-plugin.json").read_bytes()
    assert len(saved) <= MANIFEST_LIMIT and not saved.startswith(b"{\n")
    assert saved == (extracted / "sai-plugin.json").read_bytes()
    assert fixture.plugins("check", installed)["manifest"] == normalized
    assert manifest_path.read_bytes() == raw

    # 2. 【插件分发验收】【缺省膨胀】补全字段后仍超限时，不发布归档或改写安装配置
    overflow = fixture.root / "manifest-overflow"
    shutil.copytree(source, overflow)
    base["id"] = "manifest-overflow"
    too_large, normalized = boundary_manifest(base, defaults_overflow=True)
    (overflow / "sai-plugin.json").write_bytes(too_large)
    assert fixture.plugins("check", overflow)["manifest"] == normalized
    config_before = fixture.config.read_bytes()
    output = fixture.root / "rejected/overflow.tar.gz"
    assert "serialized plugin manifest exceeds 64 KiB" in fixture.plugins(
        "pack", overflow, "--output", output, succeeds=False)
    assert not output.parent.exists()
    assert "serialized plugin manifest exceeds 64 KiB" in fixture.plugins("install", overflow, succeeds=False)
    assert fixture.config.read_bytes() == config_before
    assert not (installed.parent / "manifest-overflow").exists()
    assert not any(path.name.startswith((".install-", ".backup-")) for path in installed.parent.iterdir())
    assert (overflow / "sai-plugin.json").read_bytes() == too_large
    return {"source_bytes": len(raw), "installed_bytes": len(saved),
            "overflow_source_bytes": len(too_large), "overflow_rejected": True}


def main():
    """【插件分发验收】【执行入口】解析程序和报告路径，记录断言结果、命令与耗时。"""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    arguments = parser.parse_args()
    if sys.platform != "linux":
        parser.error("This fixture uses Linux XDG isolation; other platforms are not supported.")
    binary = arguments.binary.resolve(strict=True)
    report = arguments.report.resolve()
    example = Path(__file__).resolve().parents[2] / "examples/lua-plugins/project-notes"
    started = time.monotonic()
    evidence = {"binary": str(binary), "passed": False}
    with binary.open("rb") as handle:
        evidence["binary_sha256"] = hashlib.file_digest(handle, "sha256").hexdigest()
    with tempfile.TemporaryDirectory(prefix="sai-distribution-smoke-") as directory:
        fixture = Fixture(binary, Path(directory))
        evidence["working_directory"] = str(fixture.project)
        evidence["calls"] = fixture.calls
        try:
            evidence["distribution"] = verify_distribution(fixture, example)
            evidence["manifest_boundary"] = verify_manifest_boundary(fixture)
            evidence["passed"] = True
        finally:
            evidence["seconds"] = round(time.monotonic() - started, 3)
            report.parent.mkdir(parents=True, exist_ok=True)
            report.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"Passed {len(fixture.calls)} CLI calls in {evidence['seconds']}s; report: {report}")


if __name__ == "__main__":
    main()
