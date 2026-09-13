use super::web_fetch_http_support::{response, server};
use crate::{paths::SaiPaths, plugins::private::PrivatePluginHost};
use sai_plugin_runtime::{
    Capabilities, InvocationContext, PluginManifest, PluginPackage, PluginRuntime,
};
use serde_json::json;
use std::{collections::BTreeMap, io::Write, sync::Arc, time::Duration};

/// 【归档授权测试】【能力组合】分别声明任意来源读取和插件私有工作目录
/// @param read_any 网络读取许可；workspace 为工作目录许可
/// @returns 不包含精确 HTTP 来源或其他能力的集合
fn capabilities(read_any: bool, workspace: bool) -> Capabilities {
    serde_json::from_value(json!({
        "http_read_any":read_any,"system":{"workspace":workspace}
    }))
    .unwrap()
}

/// 【归档授权测试】【外部实例】使用普通包身份和正式私有目录宿主读取归档
/// @param paths 隔离应用路径；declared 为清单声明；granted 为用户授权
/// @returns 仅提供归档读取工具的 Lua 实例
fn external(paths: &SaiPaths, declared: Capabilities, granted: Capabilities) -> PluginRuntime {
    let manifest = PluginManifest::parse(
        &json!({
            "api_version":1,"id":"external-archive","version":"1.0.0",
            "name":"External Archive","description":"Public archive contract",
            "entry":"init.lua","capabilities":declared
        })
        .to_string(),
    )
    .unwrap();
    let source = r#"
        sai.register_tool({name='extract',description='Read archive',parameters={type='object'},
            execute=function(args)
                local work = sai.workspace.open('download')
                work:extract_tar_gz({url=args.url,destination='snapshot',max_bytes=4096,
                    max_unpacked_bytes=4096,max_entries=4,timeout_ms=1000})
                return work:read_text('snapshot/report.txt').text
            end})
    "#;
    let package = PluginPackage::new(
        manifest,
        BTreeMap::from([("init.lua".into(), source.into())]),
    )
    .unwrap();
    PluginRuntime::load(
        package,
        json!({}),
        granted,
        Arc::new(PrivatePluginHost::new(paths, "external-archive")),
    )
    .unwrap()
}

/// 【归档授权测试】【固定正文】构造一个普通文件的有效压缩包
/// @returns 包含固定文字的 tar.gz 原始字节
fn archive() -> Vec<u8> {
    let body = b"public archive\n";
    let mut tar = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(body.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, "report.txt", body.as_slice())
        .unwrap();
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all(&tar.into_inner().unwrap()).unwrap();
    gzip.finish().unwrap()
}

/// 【归档授权测试】【独立交集】任一能力缺少声明或授权时均不能触达归档地址
/// @returns 无，网络许可不会开放私有工作目录，目录许可也不会开放网络
#[tokio::test]
async fn arbitrary_archive_reads_require_both_independent_capabilities() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/archive", listener.local_addr().unwrap());
    for (declared, granted, message) in [
        (
            capabilities(true, true),
            capabilities(false, true),
            "HTTP origin",
        ),
        (
            capabilities(false, true),
            capabilities(true, true),
            "HTTP origin",
        ),
        (
            capabilities(true, true),
            capabilities(true, false),
            "private workspace",
        ),
        (
            capabilities(true, false),
            capabilities(true, true),
            "private workspace",
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        let plugin = external(&SaiPaths::for_tests(root.path()), declared, granted);
        let error = plugin
            .call_tool("extract", json!({"url":url}), InvocationContext::default())
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains(message), "{error:#}");
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(20), listener.accept())
            .await
            .is_err()
    );
}

/// 【归档授权测试】【跨来源读取】外部包以任意来源授权跟随跳转并读取真实解压结果
/// @returns 无，压缩正文经过正式下载、展开、发布和目录读取路径
#[tokio::test]
async fn arbitrary_archive_reads_follow_authorized_redirects_and_extract_bytes() {
    // 1. 【归档授权测试】【网络样本】跳转地址与归档正文使用两个独立本地来源
    let (target, target_task) = server(vec![response(200, "", &archive())]).await;
    let (origin, origin_task) = server(vec![response(
        302,
        &format!("Location: {target}/archive\r\n"),
        b"",
    )])
    .await;
    // 2. 【归档授权测试】【正式调用】没有精确来源、普通文件或写入授权
    let root = tempfile::tempdir().unwrap();
    let plugin = external(
        &SaiPaths::for_tests(root.path()),
        capabilities(true, true),
        capabilities(true, true),
    );
    assert_eq!(
        plugin
            .call_tool(
                "extract",
                json!({"url":origin}),
                InvocationContext::default()
            )
            .await
            .unwrap(),
        "public archive\n"
    );
    assert_eq!(origin_task.await.unwrap().len(), 1);
    assert_eq!(target_task.await.unwrap().len(), 1);
}
