"""用仓库外普通包验证源码路径拒绝及失败更新的数据保留。"""

import argparse
import hashlib
import json
import sys
import tarfile
import tempfile
import time
from pathlib import Path

from fixture import Fixture


PLUGIN = "source-paths"
ENTRY = """local source = require("modules.check")

--- 【路径验收】【读取模块】返回加载的模块及插件自己的设置
---@param arguments string 未使用的命令参数
---@param ctx table 本次调用上下文
---@return table 模块内容和已保存设置
local function read(arguments, ctx)
    return { value = source.value, label = sai.config.label }
end

sai.register_command({name = "read", description = "Read the source fixture.", execute = read})
"""


def snapshot(directory: Path):
    """【路径验收】【文件快照】接收目录，返回全部普通文件的相对名称及完整字节。"""
    return {path.relative_to(directory).as_posix(): path.read_bytes()
            for path in directory.rglob("*") if path.is_file()}


def command(fixture: Fixture):
    """【路径验收】【实际执行】接收隔离环境，返回正式插件命令的 JSON 结果。"""
    return json.loads(fixture.plugins("run", PLUGIN, "read")["output"])


def archive_sources(result):
    """【路径验收】【归档内容】接收打包结果，返回归档中的 Lua 路径及原始字节。"""
    with tarfile.open(result["path"], "r:gz") as archive:
        return {member.name: archive.extractfile(member).read()
                for member in archive.getmembers() if member.name.endswith(".lua")}


def verify(fixture: Fixture):
    """【路径验收】【安装更新】接收隔离环境，返回路径拒绝、内容保留和修正结果。"""
    # 1. 【路径验收】【普通安装】正常嵌套模块可打包、安装、配置并执行
    source = fixture.root / "source"
    fixture.plugins("init", PLUGIN, source)
    (source / "init.lua").write_text(ENTRY, encoding="utf-8")
    (source / "modules").mkdir()
    canonical = source / "modules/check.lua"
    canonical.write_text('return { value = "original" }\n', encoding="utf-8")
    fixture.plugins("check", source)
    original_archive = fixture.plugins("pack", source)
    assert archive_sources(original_archive) == {
        f"{PLUGIN}/init.lua": ENTRY.encode(),
        f"{PLUGIN}/modules/check.lua": canonical.read_bytes(),
    }
    archive_before = Path(original_archive["path"]).read_bytes()
    installed = Path(fixture.plugins("install", source)["path"])
    settings = fixture.root / "settings.json"
    settings.write_text('{"label":"retained"}\n', encoding="utf-8")
    fixture.plugins("configure", PLUGIN, settings)
    fixture.plugins("enable", PLUGIN)
    expected = {"value": "original", "label": "retained"}
    assert command(fixture) == expected
    config_before = fixture.config.read_bytes()
    installed_before = snapshot(installed)

    # 2. 【路径验收】【冲突拒绝】反斜杠不能把另一份源码合并进现有模块
    alias = source / "modules\\check.lua"
    alias.write_text('return { value = "alias" }\n', encoding="utf-8")
    source_before = snapshot(source)
    rejected = fixture.root / "rejected/release.tar.gz"
    for arguments in [
        ("check", source),
        ("pack", source, "--output", rejected),
        ("install", source, "--replace"),
    ]:
        error = fixture.plugins(*arguments, succeeds=False)
        assert "invalid plugin source path" in error
        assert "normalized relative path" in error
    assert not rejected.parent.exists()
    assert snapshot(source) == source_before
    assert snapshot(installed) == installed_before
    assert fixture.config.read_bytes() == config_before
    assert Path(original_archive["path"]).read_bytes() == archive_before
    assert command(fixture) == expected

    # 3. 【路径验收】【修正重试】消除非法名称后，普通版本替换继续保留设置和启用状态
    alias.unlink()
    canonical.write_text('return { value = "updated" }\n', encoding="utf-8")
    manifest_path = source / "sai-plugin.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    manifest["version"] = "0.2.0"
    manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
    fixture.plugins("check", source)
    updated_archive = fixture.plugins("pack", source)
    assert archive_sources(updated_archive) == {
        f"{PLUGIN}/init.lua": ENTRY.encode(),
        f"{PLUGIN}/modules/check.lua": canonical.read_bytes(),
    }
    fixture.plugins("install", source, "--replace")
    assert command(fixture) == {"value": "updated", "label": "retained"}
    assert fixture.config.read_bytes() == config_before
    assert not any(path.name.startswith((".install-", ".backup-"))
                   for path in installed.parent.iterdir())
    fixture.plugins("remove", PLUGIN)
    assert not installed.exists()
    assert not (fixture.root / "config/sai/config.jsonc").exists()
    return {"path_collision_rejected": True, "failed_update_preserved_sources": True,
            "failed_update_preserved_config": True, "valid_update_executed": True}


def main():
    """【路径验收】【执行入口】解析程序和报告路径，保存结果、完整命令及耗时。"""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    arguments = parser.parse_args()
    if sys.platform != "linux":
        parser.error("This fixture requires Linux filenames and XDG isolation.")
    binary = arguments.binary.resolve(strict=True)
    report = arguments.report.resolve()
    evidence = {"binary": str(binary), "passed": False}
    with binary.open("rb") as stream:
        evidence["binary_sha256"] = hashlib.file_digest(stream, "sha256").hexdigest()
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="sai-source-path-smoke-") as directory:
        fixture = Fixture(binary, Path(directory))
        evidence.update({"working_directory": str(fixture.project), "calls": fixture.calls})
        try:
            evidence["verification"] = verify(fixture)
            evidence["passed"] = True
        finally:
            evidence["seconds"] = round(time.monotonic() - started, 3)
            report.parent.mkdir(parents=True, exist_ok=True)
            report.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n",
                              encoding="utf-8")
    print(f"Passed {len(fixture.calls)} CLI calls in {evidence['seconds']}s; report: {report}")


if __name__ == "__main__":
    main()
