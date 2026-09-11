#[path = "sqlite/support.rs"]
mod support;

use serde_json::json;

/// 【数据库快照测试】【基本契约】零外部授权可创建快照并查询完整整数、文字、实数与 null
/// @returns 无；所有结果来自实际 Lua 绑定
#[tokio::test]
async fn sqlite_snapshot_preserves_types_pagination_and_readonly_calls() {
    let body = format!(
        "{} return {{readonly=not ctx.allow_writes,bytes=original:len(),data=sai.sqlite.query(original,{{table='notes',columns={{'id','text','score','optional'}},order_by={{{{column='id',descending=true}}}},limit=1,offset=1}})}}",
        support::SEED
    );
    let plugin = support::plugin(&body, json!({}));
    let result = support::run(&plugin, json!({})).await.unwrap();
    assert_eq!(result["readonly"], true);
    assert!(result["bytes"].as_u64().unwrap() >= 8192);
    assert_eq!(
        result["data"]["columns"],
        json!(["id", "text", "score", "optional"])
    );
    assert_eq!(
        result["data"]["rows"],
        json!([{"id":1,"text":"第一条","score":1.25,"optional":null}])
    );
}

/// 【数据库快照测试】【原子副本】批量更新和删除产生新快照，失败不会改变原始数据
/// @returns 无；有约束冲突的整批操作不返回部分结果
#[tokio::test]
async fn sqlite_snapshot_changes_preserve_input_and_rollback_failed_batches() {
    let body = format!(
        r#"{}
        local revision=original:sha256()
        local changed=sai.sqlite.apply(original,{{
            {{op='upsert',table='notes',key='id',rows={{{{id=1,text='updated',score=7.5}}}}}},
            {{op='delete',table='notes',where={{id=2}}}}
        }},{{max_bytes=65536}})
        local ok=pcall(sai.sqlite.apply,original,{{
            {{op='delete',table='notes',where={{id=2}}}},
            {{op='insert',table='notes',rows={{{{id=1,text='duplicate'}}}}}}
        }},{{max_bytes=65536}})
        assert(not ok,'duplicate primary key was accepted')
        assert(revision==original:sha256(),'original snapshot changed')
        return {{original=sai.sqlite.query(original,{{table='notes',columns={{'id','text'}},order_by={{{{column='id'}}}}}}),
            changed=sai.sqlite.query(changed,{{table='notes',columns={{'id','text','optional'}}}})}}
        "#,
        support::SEED
    );
    let plugin = support::plugin(&body, json!({}));
    let result = support::run(&plugin, json!({})).await.unwrap();
    assert_eq!(
        result["original"]["rows"],
        json!([{"id":1,"text":"第一条"},{"id":2,"text":"second"}])
    );
    assert_eq!(
        result["changed"]["rows"],
        json!([{"id":1,"text":"updated","optional":null}])
    );
}

/// 【数据库快照测试】【输入拒绝】路径、SQL、未知动作和非法参数不能伪装为结构化操作
/// @returns 无；错误被捕获后仍可读取原始快照
#[tokio::test]
async fn sqlite_snapshot_rejects_unbounded_or_executable_requests() {
    let body = format!(
        r#"{}
        local invalid={{
            function()sai.sqlite.query('/tmp/database.db',{{table='notes',columns={{'id'}}}})end,
            function()sai.sqlite.query(original,{{table='notes; ATTACH x',columns={{'id'}}}})end,
            function()sai.sqlite.query(original,{{table='notes',columns={{'id'}},sql='SELECT 1'}})end,
            function()sai.sqlite.query(original,{{table='notes',columns={{'id'}},limit=0}})end,
            function()sai.sqlite.query(original,{{table='notes',columns={{'id'}},offset=-1}})end,
            function()sai.sqlite.apply(original,{{{{op='sql',sql='VACUUM INTO x'}}}})end,
            function()sai.sqlite.apply(original,{{{{op='delete',table='../notes'}}}})end,
            function()sai.sqlite.apply(original,{{{{op='insert',table='notes',rows={{{{id={{}}}}}}}}}})end,
            function()sai.sqlite.apply(original,{{}},{{max_bytes='65536'}})end,
            function()sai.sqlite.apply(original,{{}},{{extra=true}})end
        }}
        for _,probe in ipairs(invalid)do local ok=pcall(probe); assert(not ok,'invalid request accepted')end
        return sai.sqlite.query(original,{{table='notes',columns={{'id'}},where={{optional=sai.json.null}}}})
        "#,
        support::SEED
    );
    let plugin = support::plugin(&body, json!({}));
    assert_eq!(
        support::run(&plugin, json!({})).await.unwrap()["rows"],
        json!([{"id":1}])
    );
}

/// 【数据库快照测试】【整数和冲突】自动编号保留完整整数，部分更新不覆盖未提供的字段
/// @returns 无；布尔参数按 SQLite 整数存储，文本中的 SQL 字符不参与执行
#[tokio::test]
async fn integer_limits_autoincrement_and_partial_upserts_preserve_data() {
    let plugin = support::plugin(
        r#"
        local original=sai.sqlite.apply(nil,{
            {op='create_table',name='notes',columns={{name='id',kind='integer'},{name='large',kind='integer'},{name='flag',kind='integer'},{name='text',kind='text'}},primary_key='id',auto_increment=true},
            {op='insert',table='notes',rows={
                {large=math.maxinteger,flag=true,text="Robert'); DROP TABLE notes; --"},
                {large=math.mininteger,flag=false,text='original'}
            }}
        },{max_bytes=65536})
        local updated=sai.sqlite.apply(original,{{op='upsert',table='notes',key='id',rows={{id=2,text='updated'},{id=1}}}},{max_bytes=65536})
        local rows=sai.sqlite.query(updated,{table='notes',columns={'id','large','flag','text'},order_by={{column='id'}}}).rows
        assert(rows[1].large==math.maxinteger and rows[2].large==math.mininteger)
        assert(rows[1].text=="Robert'); DROP TABLE notes; --")
        assert(#sai.sqlite.query(updated,{table='notes',columns={'id'},where={flag=false}}).rows==1)
        return rows
    "#,
        json!({}),
    );
    assert_eq!(
        support::run(&plugin, json!({})).await.unwrap(),
        json!([
            {"id":1,"large":i64::MAX,"flag":1,"text":"Robert'); DROP TABLE notes; --"},
            {"id":2,"large":i64::MIN,"flag":0,"text":"updated"}
        ])
    );
}

/// 【数据库快照测试】【合法边界】三十二列、六十四项变更及五百一十二行均能完整处理
/// @returns 无；最大分页不静默少行，大偏移返回空数组
#[tokio::test]
async fn maximum_valid_columns_changes_and_rows_are_supported() {
    let plugin = support::plugin(
        r#"
        local columns,names,rows={},{},{}
        for index=1,32 do columns[index]={name='c'..index,kind='integer'} names[index]='c'..index end
        for index=1,512 do rows[index]={c1=index,c32=-index} end
        local changes={
            {op='create_table',name='notes',columns=columns},
            {op='insert',table='notes',rows=rows}
        }
        for index=1,62 do changes[#changes+1]={op='create_index',name='idx_'..index,table='notes',columns={'c1'}} end
        local original=sai.sqlite.apply(nil,changes,{max_bytes=1048576})
        local result=sai.sqlite.query(original,{table='notes',columns=names,order_by={{column='c1'}},limit=512})
        assert(#result.columns==32 and #result.rows==512)
        assert(result.rows[512].c1==512 and result.rows[512].c32==-512)
        return sai.sqlite.query(original,{table='notes',columns={'c1'},offset=1000000}).rows
    "#,
        json!({"instructions":8_000_000}),
    );
    assert_eq!(support::run(&plugin, json!({})).await.unwrap(), json!([]));
}
