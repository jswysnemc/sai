#[path = "sqlite/fixtures.rs"]
mod fixtures;
#[path = "sqlite/support.rs"]
mod support;

use serde_json::json;
use std::time::Duration;

/// 【数据库快照测试】【加载权限】数据库计算入口在初始化期间不可执行
/// @returns 无；加载事务失败，不留工具注册
#[test]
fn sqlite_snapshot_is_unavailable_during_initialization() {
    let error=support::load("sai.sqlite.apply(nil,{{op='create_table',name='notes',columns={{name='id',kind='integer'}}}})",json!({})).err().unwrap();
    assert!(format!("{error:#}").contains("callbacks"), "{error:#}");
}

/// 【数据库快照测试】【句柄失效】关闭与跨回调保留的句柄都不能用于查询或修改
/// @returns 无；后续回调仍能构造新快照
#[tokio::test]
async fn sqlite_handles_cannot_outlive_their_callback_or_be_reopened_after_close() {
    let body = format!(
        r#"
        if args.inspect then
            assert(not pcall(sai.sqlite.query,saved,{{table='notes',columns={{'id'}}}}))
            assert(not pcall(sai.sqlite.apply,saved,{{{{op='delete',table='notes'}}}}))
        end
        {}
        saved=original
        if args.close then
            original:close()
            assert(not pcall(sai.sqlite.query,original,{{table='notes',columns={{'id'}}}}))
        end
        return true
    "#,
        support::SEED
    );
    let plugin = support::plugin(&body, json!({}));
    assert_eq!(support::run(&plugin, json!({})).await.unwrap(), true);
    assert_eq!(
        support::run(&plugin, json!({"inspect":true,"close":true}))
            .await
            .unwrap(),
        true
    );
    assert_eq!(support::run(&plugin, json!({})).await.unwrap(), true);
}

/// 【数据库快照测试】【系统配额】数据库与其他二进制接口共用系统调用额度
/// @returns 无；失败后下一回调重新取得额度
#[tokio::test]
async fn sqlite_operations_share_the_system_call_quota() {
    let body = format!(
        r#"{}
        local ok,message=pcall(sai.sqlite.query,original,{{table='notes',columns={{'id'}}}})
        assert(not ok and tostring(message):find('system call budget',1,true),tostring(message))
        return true
    "#,
        support::SEED
    );
    let plugin = support::plugin(&body, json!({"system_calls":1}));
    for _ in 0..2 {
        assert_eq!(support::run(&plugin, json!({})).await.unwrap(), true);
    }
}

/// 【数据库快照测试】【计算额度】修改公开预算字段不能扩大原生复制或 SQLite 执行额度
/// @returns 无；低预算明确失败，其他实例仍可正常执行
#[tokio::test]
async fn sqlite_computation_uses_the_trusted_instruction_budget() {
    let plugin = support::plugin(
        &format!(
            "sai.limits.instructions=1000000000 {} return true",
            support::SEED
        ),
        json!({"instructions":10000}),
    );
    let error = support::run(&plugin, json!({})).await.unwrap_err();
    assert!(
        format!("{error:#}").contains("instruction budget"),
        "{error:#}"
    );
    let other = support::plugin(&format!("{} return true", support::SEED), json!({}));
    assert_eq!(support::run(&other, json!({})).await.unwrap(), true);
}

/// 【数据库快照测试】【排队验证】占满阻塞线程池，验证局部超时与工作预算恢复
/// @param body 本次操作；args 为输入；binary_bytes 为恰好占满的总预算
/// @returns 无；排队任务退出前不能构造额外缓冲
fn queued_timeout(body: &str, args: serde_json::Value, binary_bytes: usize) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            entered_tx.send(()).unwrap();
            let _ = release_rx.recv();
        });
        entered_rx.await.unwrap();
        let source = format!(r#"
            if args.recover then return sai.binary.from_bytes('x'):len() end
            {body}
            local available,reason=pcall(sai.binary.from_bytes,'x')
            assert(not available and tostring(reason):find('size limit',1,true),tostring(reason))
            return true
        "#);
        let plugin = support::plugin(&source, json!({
            "binary_bytes":binary_bytes,"output_bytes":32768,"binary_timeout_ms":20,"timeout_ms":1000
        }));
        let result = support::run(&plugin, args).await;
        release_tx.send(()).unwrap();
        blocker.await.unwrap();
        assert_eq!(result.unwrap(), true);
        let mut recovered = false;
        for _ in 0..50 {
            if support::run(&plugin, json!({"recover":true})).await.is_ok() {
                recovered = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        assert!(recovered);
    });
}

/// 【数据库快照测试】【修改排队】局部超时不能提前归还排队修改持有的缓冲额度
/// @returns 无；请求不能扩大清单时限，迟到任务不执行数据库操作
#[test]
fn timed_out_sqlite_workers_keep_their_reservations_until_they_exit() {
    queued_timeout(
        r#"
        local ok,message=pcall(sai.sqlite.apply,nil,{{op='create_table',name='notes',columns={{name='id',kind='integer'}}}},{max_bytes=8192,timeout_ms=1000000})
        assert(not ok and tostring(message):find('timed out',1,true),tostring(message))
    "#,
        json!({}),
        1024 * 1024 + 3 * 8192,
    );
}

/// 【数据库快照测试】【查询排队】查询超时并关闭原句柄后，排队线程仍持有输入及工作额度
/// @returns 无；线程退出后原实例恢复，查询不会交付迟到结果
#[test]
fn timed_out_queries_keep_the_source_alive_until_the_worker_exits() {
    let bytes = fixtures::snapshot("CREATE TABLE notes(id INTEGER);");
    queued_timeout(
        r#"
        local original=sai.binary.decode_base64(args.data)
        local ok,message=pcall(sai.sqlite.query,original,{table='notes',columns={'id'}})
        assert(not ok and tostring(message):find('timed out',1,true),tostring(message))
        original:close()
    "#,
        json!({"data":fixtures::encoded(&bytes)}),
        1024 * 1024 + 3 * bytes.len() + 32768,
    );
}
