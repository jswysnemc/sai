#[path = "sqlite/fixtures.rs"]
mod fixtures;
#[path = "sqlite/support.rs"]
#[allow(dead_code)]
mod support;

use serde_json::json;

/// 【数据库快照测试】【执行中断】原生全表扫描也消耗指令，额度不足时不会返回截断结果
/// @returns 无；同样的镜像和预算可完成有界首行查询，失败后下一回调恢复
#[tokio::test]
async fn sqlite_vm_instructions_are_metered_beyond_snapshot_copying() {
    let bytes=fixtures::snapshot("CREATE TABLE notes(id INTEGER PRIMARY KEY,text TEXT); WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<10000) INSERT INTO notes SELECT i,'present' FROM n;");
    let plugin = support::plugin(
        r#"
        local data=sai.binary.decode_base64(args.data)
        local filter=args.scan and {text='missing'} or {}
        return sai.sqlite.query(data,{table='notes',columns={'id'},where=filter,limit=1})
    "#,
        json!({"instructions":bytes.len()+8000}),
    );
    let args = json!({"data":fixtures::encoded(&bytes)});
    assert_eq!(
        support::run(&plugin, args.clone()).await.unwrap()["rows"],
        json!([{"id":1}])
    );
    let mut slow = args.clone();
    slow["scan"] = json!(true);
    let error = support::run(&plugin, slow).await.unwrap_err();
    assert!(
        format!("{error:#}").contains("instruction budget"),
        "{error:#}"
    );
    assert_eq!(
        support::run(&plugin, args).await.unwrap()["rows"],
        json!([{"id":1}])
    );
}

/// 【数据库快照测试】【容量恢复】超过固定页容量的事务全部失败，反复失败也归还工作额度
/// @returns 无；原始缓冲保持原样，后续小事务仍可完成
#[tokio::test]
async fn failed_growth_releases_reservations_and_keeps_the_original_snapshot() {
    let plugin = support::plugin(
        r#"
        local original=sai.sqlite.apply(nil,{{op='create_table',name='notes',columns={{name='id',kind='integer'},{name='text',kind='text'}}}},{max_bytes=8192})
        local revision=original:sha256()
        for attempt=1,8 do
            local ok,message=pcall(sai.sqlite.apply,original,{{op='insert',table='notes',rows={{id=attempt,text=string.rep('x',8192)}}}},{max_bytes=8192})
            assert(not ok and tostring(message):find('full',1,true),tostring(message))
        end
        assert(revision==original:sha256())
        local updated=sai.sqlite.apply(original,{{op='insert',table='notes',rows={{id=1,text='fits'}}}},{max_bytes=8192})
        return sai.sqlite.query(updated,{table='notes',columns={'text'}}).rows
    "#,
        json!({"binary_bytes":2*1024*1024,"output_bytes":32768}),
    );
    assert_eq!(
        support::run(&plugin, json!({})).await.unwrap(),
        json!([{"text":"fits"}])
    );
}

/// 【数据库快照测试】【输出限额】结果按完整 JSON 大小计量，失败不会交付首行或占用剩余额度
/// @returns 无；收窄为单行后原实例可继续查询
#[tokio::test]
async fn oversized_json_results_fail_atomically_and_allow_smaller_queries() {
    let plugin = support::plugin(
        r#"
        local original=sai.sqlite.apply(nil,{{op='create_table',name='notes',columns={{name='id',kind='integer'},{name='text',kind='text'}}}},{max_bytes=16384})
        for id=1,2 do
            local updated=sai.sqlite.apply(original,{{op='insert',table='notes',rows={{id=id,text=string.rep('x',600)}}}},{max_bytes=16384})
            original:close()
            original=updated
        end
        for attempt=1,4 do
            local ok,message=pcall(sai.sqlite.query,original,{table='notes',columns={'id','text'}})
            assert(not ok and tostring(message):find('output limit',1,true),tostring(message))
        end
        return #sai.sqlite.query(original,{table='notes',columns={'text'},limit=1}).rows[1].text
    "#,
        json!({"output_bytes":1024,"binary_bytes":2*1024*1024}),
    );
    assert_eq!(support::run(&plugin, json!({})).await.unwrap(), 600);
}

/// 【数据库快照测试】【请求限额】JSON 转义后的总字节与一 MiB 硬上限都不能绕过
/// @returns 无；合法的较小请求仍能正常创建并查询
#[tokio::test]
async fn request_limits_count_json_escaping_and_have_an_absolute_ceiling() {
    for (limits, body) in [
        (
            json!({"output_bytes":1024}),
            "string.rep(string.char(0),180)",
        ),
        (
            json!({"output_bytes":4*1024*1024,"instructions":4_000_000}),
            "string.rep('x',1048576)",
        ),
    ] {
        let plugin=support::plugin(&r#"
            local ok,message=pcall(sai.sqlite.apply,nil,{{op='insert',table='notes',rows={{text=TEST_VALUE}}}})
            assert(not ok and tostring(message):find('input limit',1,true),tostring(message))
            return sai.sqlite.apply(nil,{{op='create_table',name='notes',columns={{name='id',kind='integer'}}}},{max_bytes=8192}):len()
        "#.replace("TEST_VALUE",body),limits);
        assert_eq!(support::run(&plugin, json!({})).await.unwrap(), 8192);
    }
}

/// 【数据库快照测试】【工作预留】查询必须先取得输入副本、页缓存及结果的共同额度
/// @returns 无；余量少一个字节也失败，充足额度时返回完整数据
#[tokio::test]
async fn query_working_memory_and_output_share_the_binary_budget() {
    let bytes = fixtures::snapshot("CREATE TABLE notes(id INTEGER); INSERT INTO notes VALUES(1);");
    let required = 3 * bytes.len() + 2 * 1024 * 1024;
    for (limit, success) in [(required - 1, false), (required, true)] {
        let plugin=support::plugin("local data=sai.binary.decode_base64(args.data) return sai.sqlite.query(data,{table='notes',columns={'id'}})",json!({"binary_bytes":limit}));
        let result = support::run(&plugin, json!({"data":fixtures::encoded(&bytes)})).await;
        if success {
            assert_eq!(result.unwrap()["rows"], json!([{"id":1}]));
        } else {
            assert!(format!("{:#}", result.unwrap_err()).contains("size limit"));
        }
    }
}
