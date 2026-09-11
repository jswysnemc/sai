#[path = "sqlite/fixtures.rs"]
mod fixtures;
#[path = "sqlite/support.rs"]
mod support;

use serde_json::json;

const REJECT_IMAGE: &str = r#"
    local original=sai.binary.decode_base64(args.data)
    local revision=original:sha256()
    for _, operation in ipairs({
        function() return sai.sqlite.query(original,{table='notes',columns={'id'}}) end,
        function() return sai.sqlite.apply(original,{{op='delete',table='notes'}},{max_bytes=1048576}) end
    }) do
        local ok,message=pcall(operation)
        assert(not ok,'invalid snapshot accepted')
        if args.reason then assert(tostring(message):find(args.reason,1,true),tostring(message)) end
    end
    assert(revision==original:sha256(),'input changed after rejection')
    return true
"#;

/// 【数据库快照测试】【镜像损坏】空、截断、损坏页和 WAL 头不能交给查询或修改
/// @returns 无；每种错误都保留原始缓冲
#[tokio::test]
async fn malformed_and_wal_snapshots_fail_without_changing_the_input() {
    let bytes = fixtures::snapshot(
        "CREATE TABLE notes(id INTEGER PRIMARY KEY); INSERT INTO notes VALUES(1);",
    );
    let mut wal = bytes.clone();
    wal[18] = 2;
    wal[19] = 2;
    let mut page = bytes.clone();
    page[100] = 0;
    let mut page_size = bytes.clone();
    page_size[16] = 3;
    let plugin = support::plugin(REJECT_IMAGE, json!({"instructions":4_000_000}));
    for data in [
        Vec::new(),
        vec![0; 100],
        bytes[..4096].to_vec(),
        bytes[..bytes.len() - 1].to_vec(),
        wal,
        page,
        page_size,
    ] {
        assert_eq!(
            support::run(&plugin, json!({"data":fixtures::encoded(&data)}))
                .await
                .unwrap(),
            true
        );
    }
}

/// 【数据库快照测试】【附属执行】包含视图、触发器、虚表、生成列或外键的目标不能执行
/// @returns 无；普通表也不能掩盖同库附属程序
#[tokio::test]
async fn executable_schema_objects_and_implicit_column_updates_are_rejected() {
    let plugin = support::plugin(REJECT_IMAGE, json!({"instructions":4_000_000}));
    for sql in [
        "CREATE TABLE notes(id INTEGER); CREATE VIEW other AS SELECT id FROM notes;",
        "CREATE TABLE notes(id INTEGER); CREATE TRIGGER changed AFTER DELETE ON notes BEGIN INSERT INTO notes VALUES(2); END;",
        "CREATE VIRTUAL TABLE notes USING fts5(id);",
        "CREATE TABLE notes(id INTEGER, calculated INTEGER GENERATED ALWAYS AS (id+1));",
        "CREATE TABLE parent(id INTEGER PRIMARY KEY); CREATE TABLE notes(id INTEGER REFERENCES parent(id));",
    ] {
        let data=fixtures::snapshot(sql);
        assert_eq!(support::run(&plugin,json!({"data":fixtures::encoded(&data)})).await.unwrap(),true,"{sql}");
    }
}

/// 【数据库快照测试】【输出类型】BLOB、无效 UTF-8、非有限实数及超限文字不返回部分行
/// @returns 无；只读取合法列仍然成功
#[tokio::test]
async fn unsupported_sqlite_values_cannot_be_silently_coerced_or_truncated() {
    let plugin = support::plugin(
        r#"
        local original=sai.binary.decode_base64(args.data)
        local ok,message=pcall(sai.sqlite.query,original,{table='notes',columns={'id','text'}})
        assert(not ok,'unsupported result was accepted')
        assert(tostring(message):find(args.reason,1,true),tostring(message))
        return sai.sqlite.query(original,{table='notes',columns={'id'}}).rows[1].id
    "#,
        json!({}),
    );
    for (expression, reason) in [
        ("X'ff'", "blob"),
        ("CAST(X'ff' AS TEXT)", "utf-8"),
        ("1e999", "finite"),
        ("replace(hex(zeroblob(131073)),'0','x')", "256 KiB"),
    ] {
        let data = fixtures::snapshot(&format!(
            "CREATE TABLE notes(id INTEGER, text); INSERT INTO notes VALUES(1,{expression});"
        ));
        assert_eq!(
            support::run(
                &plugin,
                json!({"data":fixtures::encoded(&data),"reason":reason})
            )
            .await
            .unwrap(),
            1
        );
    }
}

/// 【数据库快照测试】【标量精度】非有限 Lua 数字不能变成 null 后参与删除或写入
/// @returns 无；完整有符号整数和普通布尔参数仍可使用
#[tokio::test]
async fn nonfinite_parameters_do_not_become_null_predicates() {
    let plugin = support::plugin(
        &format!(
            r#"{}
        for _,value in ipairs({{0/0,math.huge,-math.huge}}) do
            local ok=pcall(sai.sqlite.apply,original,{{{{op='delete',table='notes',where={{optional=value}}}}}},{{max_bytes=65536}})
            assert(not ok,'nonfinite predicate became SQL null')
            assert(not pcall(sai.sqlite.query,original,{{table='notes',columns={{'id'}},where={{score=value}}}}))
        end
        return #sai.sqlite.query(original,{{table='notes',columns={{'id'}}}}).rows
    "#,
            support::SEED
        ),
        json!({}),
    );
    assert_eq!(support::run(&plugin, json!({})).await.unwrap(), 2);
}

/// 【数据库快照测试】【结构数量】输入及新增结构都受同一对象总数限制
/// @returns 无；越界批次不改变已有的一百二十八个表和索引
#[tokio::test]
async fn schema_object_limits_apply_before_queries_and_after_changes() {
    let mut sql = String::from("CREATE TABLE notes(id INTEGER);");
    for index in 1..128 {
        sql.push_str(&format!("CREATE TABLE t{index}(id INTEGER);"));
    }
    let data = fixtures::snapshot(&sql);
    let plugin = support::plugin(
        r#"
        local original=sai.binary.decode_base64(args.data)
        assert(#sai.sqlite.query(original,{table='notes',columns={'id'}}).rows==0)
        local ok,message=pcall(sai.sqlite.apply,original,{{op='create_index',name='extra',table='notes',columns={'id'}}},{max_bytes=1048576})
        assert(not ok and tostring(message):find('128',1,true),tostring(message))
        return #sai.sqlite.query(original,{table='notes',columns={'id'}}).rows
    "#,
        json!({"instructions":8_000_000}),
    );
    assert_eq!(
        support::run(&plugin, json!({"data":fixtures::encoded(&data)}))
            .await
            .unwrap(),
        0
    );
    sql.push_str("CREATE TABLE extra(id INTEGER);");
    let plugin = support::plugin(REJECT_IMAGE, json!({"instructions":4_000_000}));
    assert_eq!(
        support::run(
            &plugin,
            json!({"data":fixtures::encoded(&fixtures::snapshot(&sql)),"reason":"128"})
        )
        .await
        .unwrap(),
        true
    );
}

/// 【数据库快照测试】【声明边界】列、行、批次、过滤器和排序参数必须完整校验
/// @returns 无；所有拒绝都发生在输入快照变化之前
#[tokio::test]
async fn structural_request_limits_reject_the_entire_batch() {
    let plugin = support::plugin(
        &format!(
            r#"{}
        local invalid={{
            function() return sai.sqlite.query(original,args.query) end,
            function() return sai.sqlite.apply(original,args.changes,{{max_bytes=65536}}) end
        }}
        assert(not pcall(invalid[args.query and 1 or 2]),'invalid request accepted')
        return #sai.sqlite.query(original,{{table='notes',columns={{'id'}}}}).rows
    "#,
            support::SEED
        ),
        json!({}),
    );
    let mut requests = vec![
        json!({"query":{"table":"sqlite_schema","columns":["type"]}}),
        json!({"query":{"table":"notes","columns":["id","ID"]}}),
        json!({"query":{"table":"notes","columns":["unknown"]}}),
        json!({"query":{"table":"notes","columns":["id"],"limit":513}}),
        json!({"query":{"table":"notes","columns":["id"],"offset":1000001}}),
        json!({"query":{"table":"notes","columns":["id"],"order_by":[{"column":"id","extra":true}]}}),
        json!({"query":{"table":"notes","columns":["id"],"where":{"text":"x".repeat(262145)}}}),
        json!({"changes":[{"op":"create_table","name":"x","columns":[{"name":"id","kind":"text"}],"primary_key":"id","auto_increment":true}]}),
        json!({"changes":[{"op":"create_table","name":"x","columns":[{"name":"id","kind":"integer"}],"primary_key":"missing"}]}),
        json!({"changes":[{"op":"upsert","table":"notes","key":"id","rows":[{"id":null,"text":"x"}]}]}),
        json!({"changes":[{"op":"insert","table":"notes","rows":[{"id":3,"ID":4,"text":"x"}]}]}),
        json!({"changes":[{"op":"insert","table":"notes","rows":[]}]}),
        json!({"changes":[]}),
    ];
    requests.push(json!({"query":{"table":"notes","columns":(0..33).map(|i|format!("c{i}")).collect::<Vec<_>>()}}));
    requests.push(json!({"query":{"table":"notes","columns":["x".repeat(65)]}}));
    requests.push(json!({"changes":vec![json!({"op":"delete","table":"notes"});65]}));
    requests.push(json!({"changes":[
        {"op":"insert","table":"notes","rows":vec![json!({"id":3,"text":"x"});256]},
        {"op":"insert","table":"notes","rows":vec![json!({"id":4,"text":"x"});257]}
    ]}));
    for request in requests {
        assert_eq!(support::run(&plugin, request).await.unwrap(), 2);
    }
}
