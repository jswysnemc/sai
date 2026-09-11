use super::binary_read_support::{configured, context, runtime, SOURCE};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// 【二进制读取测试】【原始字节】真实文件读取完整保留零字节和非法 UTF-8，哈希保持一致
/// @returns 无；读取结果来自有界二进制缓冲，文件内容不经过文本转换
#[tokio::test]
async fn binary_reads_preserve_bytes_that_text_cannot_represent() {
    let root = tempfile::tempdir().unwrap();
    let bytes = [0, 255, 128, 10, 13, 65, 0, 254];
    std::fs::write(root.path().join("sample.bin"), bytes).unwrap();
    let plugin = runtime(root.path(), &["."]);
    let output = plugin
        .call_tool("read", json!({"path":"sample.bin"}), context(root.path()))
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&output).unwrap(),
        json!({
            "length":bytes.len(),"sha256":format!("{:x}",Sha256::digest(bytes)),"bytes":bytes,
        })
    );
    assert_eq!(
        std::fs::read(root.path().join("sample.bin")).unwrap(),
        bytes
    );
}

/// 【二进制读取测试】【空文件与上限】空文件、恰好达到上限和超出上限使用完整读取契约
/// @returns 无；超限返回错误，不提供截断缓冲，随后可以继续读取
#[tokio::test]
async fn binary_reads_accept_empty_and_exact_files_but_reject_oversized_files() {
    let root = tempfile::tempdir().unwrap();
    for (name, size) in [("empty", 0), ("exact", 1024), ("oversized", 1025)] {
        std::fs::write(root.path().join(name), vec![0; size]).unwrap();
    }
    let plugin = configured(root.path(), &["."], SOURCE, |manifest| {
        manifest.limits.binary_bytes = 1024
    });
    for name in ["empty", "exact", "oversized", "exact"] {
        let result = plugin
            .call_tool("read", json!({"path":name}), context(root.path()))
            .await;
        if name == "oversized" {
            let error = result.unwrap_err();
            assert!(format!("{error:#}").contains("size limit"), "{error:#}");
        } else {
            let result: Value = serde_json::from_str(&result.unwrap()).unwrap();
            assert_eq!(result["length"], if name == "empty" { 0 } else { 1024 });
        }
    }
    let error = plugin
        .call_tool(
            "read",
            json!({"path":"exact","options":{"max_bytes":1023}}),
            context(root.path()),
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("size limit"), "{error:#}");
}

/// 【二进制读取测试】【大文件隔离】原始图片可以超过文字输出额度，默认读取上限仍为一 MiB
/// @returns 无；显式扩大读取上限后只返回长度、摘要和少量字节
#[tokio::test]
async fn binary_files_can_exceed_text_limits_without_expanding_the_result_channel() {
    let root = tempfile::tempdir().unwrap();
    let bytes = vec![255; 2 * 1024 * 1024 + 17];
    std::fs::write(root.path().join("large.bin"), &bytes).unwrap();
    let plugin = configured(root.path(), &["."], SOURCE, |manifest| {
        manifest.limits.output_bytes = 1024;
        manifest.limits.binary_bytes = 3 * 1024 * 1024;
    });
    let error = plugin
        .call_tool("read", json!({"path":"large.bin"}), context(root.path()))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("size limit"), "{error:#}");
    let output = plugin
        .call_tool(
            "read",
            json!({"path":"large.bin","options":{"max_bytes":bytes.len()}}),
            context(root.path()),
        )
        .await
        .unwrap();
    assert!(output.len() < 1024);
    let output: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(output["length"], bytes.len());
    assert_eq!(output["sha256"], format!("{:x}", Sha256::digest(&bytes)));
}

/// 【二进制读取测试】【文件范围】单文件授权不能读取相邻文件，目录授权不能按字符串前缀扩大
/// @returns 无；越界和缺失文件都返回错误，允许范围内的合法名称保持原样
#[tokio::test]
async fn binary_file_paths_preserve_file_grants_and_directory_boundaries() {
    let root = tempfile::tempdir().unwrap();
    for directory in ["allowed", "allowed-other", "allow~ed"] {
        std::fs::create_dir(root.path().join(directory)).unwrap();
        std::fs::write(root.path().join(directory).join("图片~.bin"), [0, 255]).unwrap();
    }
    std::fs::write(root.path().join("allowed/sibling.bin"), [1, 2]).unwrap();
    let plugin = runtime(root.path(), &["allowed/图片~.bin"]);
    plugin
        .call_tool(
            "read",
            json!({"path":"allowed/图片~.bin"}),
            context(root.path()),
        )
        .await
        .unwrap();
    for path in [
        "allowed/sibling.bin",
        "allowed-other/图片~.bin",
        "allowed/../allowed-other/图片~.bin",
        "allowed/missing.bin",
    ] {
        assert!(
            plugin
                .call_tool("read", json!({"path":path}), context(root.path()))
                .await
                .is_err(),
            "{path}"
        );
    }
    let plugin = runtime(root.path(), &["allow~ed"]);
    plugin
        .call_tool(
            "read",
            json!({"path":"allow~ed/图片~.bin"}),
            context(root.path()),
        )
        .await
        .unwrap();
    assert!(plugin
        .call_tool(
            "read",
            json!({"path":root.path().join("allowed/图片~.bin")}),
            context(root.path())
        )
        .await
        .is_err());
    assert!(plugin
        .call_tool(
            "read",
            json!({"path":"allow~ed/missing.bin"}),
            context(root.path())
        )
        .await
        .is_err());
}

/// 【二进制读取测试】【符号链接】支持初始授权范围内的链接，文件与目录链接均不能越界
/// @returns 无；越界目标没有内容进入 Lua
#[cfg(unix)]
#[tokio::test]
async fn binary_read_links_must_resolve_inside_the_granted_roots() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("allowed/nested")).unwrap();
    std::fs::write(root.path().join("allowed/nested/image.bin"), [0, 255]).unwrap();
    std::fs::write(outside.path().join("secret.bin"), [7, 9]).unwrap();
    symlink("nested/image.bin", root.path().join("allowed/image-link")).unwrap();
    symlink("nested", root.path().join("allowed/directory-link")).unwrap();
    symlink(
        outside.path().join("secret.bin"),
        root.path().join("allowed/escape-file"),
    )
    .unwrap();
    symlink(outside.path(), root.path().join("allowed/escape-dir")).unwrap();
    symlink("loop", root.path().join("allowed/loop")).unwrap();
    let plugin = runtime(root.path(), &["allowed"]);
    for path in ["allowed/image-link", "allowed/directory-link/image.bin"] {
        let output = plugin
            .call_tool("read", json!({"path":path}), context(root.path()))
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&output).unwrap()["bytes"],
            json!([0, 255])
        );
    }
    for path in [
        "allowed/escape-file",
        "allowed/escape-dir/secret.bin",
        "allowed/escape-dir/missing.bin",
        "allowed/loop",
    ] {
        assert!(
            plugin
                .call_tool("read", json!({"path":path}), context(root.path()))
                .await
                .is_err(),
            "{path}"
        );
    }
}

/// 【二进制读取测试】【特殊文件】目录、FIFO、套接字和设备都不能作为普通文件读取
/// @returns 无；没有管道写入端时读取仍会及时拒绝
#[cfg(unix)]
#[tokio::test]
async fn binary_reads_reject_special_files_without_waiting_for_fifo_writers() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("directory")).unwrap();
    assert!(std::process::Command::new("mkfifo")
        .arg(root.path().join("pipe"))
        .status()
        .unwrap()
        .success());
    let _socket = std::os::unix::net::UnixListener::bind(root.path().join("socket")).unwrap();
    let plugin = runtime(root.path(), &[".", "/dev/null"]);
    for path in ["directory", "pipe", "socket", "/dev/null"] {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            plugin.call_tool(
                "read",
                json!({"path":path,"options":{"timeout_ms":500}}),
                context(root.path()),
            ),
        )
        .await
        .unwrap();
        let error = result.unwrap_err();
        assert!(
            !format!("{error:#}").contains("timed out"),
            "{path}: {error:#}"
        );
    }
}

/// 【二进制读取测试】【文件复制】真实读取可以组合哈希与原子输出，但读取授权不会间接授予写入
/// @returns 无；只读调用不创建文件，完整授权后输出逐字节一致
#[tokio::test]
async fn binary_file_buffers_compose_with_hashing_and_independent_write_permissions() {
    let root = tempfile::tempdir().unwrap();
    let bytes = [0, 255, 128, 65, 13, 10];
    std::fs::write(root.path().join("source.bin"), bytes).unwrap();
    let source = r#"
        --- 【二进制读取测试】【复制请求】读取原始字节后尝试使用独立写入能力
        --- @return table 摘要、写入结果和错误
        local function copy()
            local data = sai.binary.read_file("source.bin")
            local digest = data:sha256()
            local ok, result = pcall(data.write,data,"output/copy.bin")
            data:close()
            return {sha256=digest,written=ok,result=ok and result or tostring(result)}
        end
        sai.register_tool({name="copy",description="Copy binary file",access="optional_writes",parameters={type="object"},execute=copy})
    "#;
    for (granted, writable) in [(true, false), (false, true), (true, true)] {
        let plugin = configured(root.path(), &["source.bin"], source, |manifest| {
            if granted {
                manifest
                    .capabilities
                    .binary
                    .write_paths
                    .insert("output".into());
            }
        });
        let mut invocation = context(root.path());
        invocation.allow_writes = writable;
        let output = plugin
            .call_tool("copy", json!({}), invocation)
            .await
            .unwrap();
        let output: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(output["sha256"], format!("{:x}", Sha256::digest(bytes)));
        assert_eq!(output["written"], granted && writable);
        assert_eq!(
            root.path().join("output/copy.bin").exists(),
            granted && writable
        );
    }
    assert_eq!(
        std::fs::read(root.path().join("output/copy.bin")).unwrap(),
        bytes
    );
}
