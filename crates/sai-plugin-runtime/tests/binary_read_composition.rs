mod common;
#[path = "binary/read_support.rs"]
mod support;
#[path = "common/vision.rs"]
mod vision;

use base64::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use support::*;

/// 【二进制读取测试】【图片组合】读取缓冲直接参与视觉、哈希和文件写入，不进行文本中转
/// @returns 无；所有下游接口收到相同完整字节，工具结果不包含图片正文
#[tokio::test]
async fn local_file_buffers_compose_with_vision_hashing_and_writing() {
    let bytes = base64::engine::general_purpose::STANDARD.decode(
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jE1sAAAAASUVORK5CYII=",
    ).unwrap();
    let host = Arc::new(Host::default());
    *host.body.lock().unwrap() = bytes.clone();
    let services = Arc::new(vision::Services::default());
    *services.response.lock().unwrap() = "fixture image description".into();
    let mut caps = capabilities();
    caps.vision = true;
    caps.binary.write_paths.insert("output".into());
    let plugin = load(r#"
        --- 【二进制读取测试】【图片链路】缓冲只通过受控方法进入视觉和写入接口
        --- @return table 文件、摘要及模型文字结果
        local function read()
            local data = sai.binary.read_file("allowed/image.png")
            assert(not pcall(sai.json.encode,data))
            local analysis = data:analyze_image({prompt="Describe the image",mime_type="image/png"})
            local result = {file=data:write("output/copy.png"),sha256=data:sha256(),description=analysis.content}
            data:close()
            return result
        end
        sai.register_tool({name="read",description="Local image composition",access="writes",parameters={type="object"},execute=read})
    "#, host.clone(), caps.clone(), caps, |_| {}).unwrap();
    let mut invocation = context();
    invocation.allow_writes = true;
    invocation.services = Some(services.clone());
    let output = plugin
        .call_tool("read", json!({}), invocation)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&output).unwrap(),
        json!({
            "file":{"path":"output/copy.png","bytes":bytes.len()},
            "sha256":format!("{:x}",Sha256::digest(&bytes)),"description":"fixture image description",
        })
    );
    let requests = services.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].1, bytes);
    let writes = host.writes.lock().unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].1, bytes);
    assert_eq!(writes[0].2.workdir, "/trusted");
}

/// 【二进制读取测试】【视觉授权】本地文件读取能力不能间接开放视觉模型
/// @returns 无；拒绝后缓冲仍可读取且模型服务没有收到请求
#[tokio::test]
async fn read_path_grants_do_not_implicitly_enable_vision() {
    let host = Arc::new(Host::default());
    *host.body.lock().unwrap() = vec![0, 255];
    let services = Arc::new(vision::Services::default());
    let plugin = runtime(
        r#"
        --- 【二进制读取测试】【独立视觉】文件读取与模型访问分别授权
        --- @return integer 仍可使用的缓冲长度
        local function read()
            local data = sai.binary.read_file("allowed/a")
            local ok, message = pcall(data.analyze_image,data,{prompt="Describe",mime_type="image/png"})
            assert(not ok and tostring(message):find("not allowed",1,true), tostring(message))
            return data:len()
        end
        sai.register_tool({name="read",description="Independent vision grant",parameters={type="object"},execute=read})
    "#,
        host,
        |_| {},
    );
    let mut invocation = context();
    invocation.services = Some(services.clone());
    assert_eq!(
        plugin
            .call_tool("read", json!({}), invocation)
            .await
            .unwrap(),
        "2"
    );
    assert!(services.requests.lock().unwrap().is_empty());
}
