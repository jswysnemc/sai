use serde::de::{IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::Deserializer;
use serde_json::value::RawValue;
use std::borrow::Cow;
use std::fmt;

/// 【插件二进制】【JSON 选取】只借用目标字段，跳过其他值，避免将大响应复制成完整对象树。
/// @param bytes 有界 JSON 正文；pointer 为 RFC 6901 路径
/// @returns 目标原始 JSON；缺失字段返回 None
pub(super) fn select<'a>(bytes: &'a [u8], pointer: &str) -> mlua::Result<Option<&'a RawValue>> {
    if pointer.len() > 1024 || (!pointer.is_empty() && !pointer.starts_with('/')) {
        return Err(mlua::Error::runtime("invalid binary JSON pointer"));
    }
    let mut value: &RawValue = serde_json::from_slice(bytes)
        .map_err(|_| mlua::Error::runtime("invalid binary JSON response"))?;
    if pointer.is_empty() {
        return Ok(Some(value));
    }
    for (depth, token) in pointer[1..].split('/').enumerate() {
        if depth >= 64 || token.split('~').skip(1).any(|s| !s.starts_with(['0', '1'])) {
            return Err(mlua::Error::runtime("invalid binary JSON pointer"));
        }
        let token = token.replace("~1", "/").replace("~0", "~");
        if !matches!(kind(value), "object" | "array") {
            return Ok(None);
        }
        let found = serde_json::Deserializer::from_str(value.get())
            .deserialize_any(Field(&token))
            .map_err(|_| mlua::Error::runtime("invalid binary JSON field"))?;
        let Some(found) = found else {
            return Ok(None);
        };
        value = found;
    }
    Ok(Some(value))
}

struct Field<'a>(&'a str);

impl<'de> Visitor<'de> for Field<'_> {
    type Value = Option<&'de RawValue>;

    /// 【插件二进制】【JSON 类型】说明字段选取要求的容器类型。
    /// @param formatter 错误输出；@returns 格式化结果
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a JSON object or array")
    }

    /// 【插件二进制】【对象字段】逐个跳过非目标字段，重复键采用最后一个值。
    /// @param map JSON 映射；@returns 借用的目标字段
    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
        let mut found = None;
        while let Some(key) = map.next_key::<Cow<'de, str>>()? {
            if key == self.0 {
                found = Some(map.next_value()?);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(found)
    }

    /// 【插件二进制】【数组字段】按规范十进制索引选取，其他元素只验证不分配。
    /// @param sequence JSON 数组；@returns 借用的目标元素
    fn visit_seq<S: SeqAccess<'de>>(self, mut sequence: S) -> Result<Self::Value, S::Error> {
        let index = self
            .0
            .parse::<usize>()
            .ok()
            .filter(|n| n.to_string() == self.0);
        let mut current = 0;
        let mut found = None;
        loop {
            if Some(current) == index {
                match sequence.next_element()? {
                    Some(value) => found = Some(value),
                    None => break,
                }
            } else if sequence.next_element::<IgnoredAny>()?.is_none() {
                break;
            }
            current += 1;
        }
        Ok(found)
    }
}

/// 【插件二进制】【JSON 类型】识别已经通过语法验证的值。
/// @param raw 已验证 JSON；@returns 稳定的类型名称
pub(super) fn kind(raw: &RawValue) -> &'static str {
    match raw.get().as_bytes()[0] {
        b'{' => "object",
        b'[' => "array",
        b'"' => "string",
        b'n' => "null",
        b't' | b'f' => "boolean",
        _ => "number",
    }
}

/// 【插件二进制】【字符串读取】普通字符串直接借用，转义字符串在有界空间内解码。
/// @param raw JSON 字符串；limit 为允许解码的字节上限
/// @returns 文本借用或解码副本
pub(super) fn string(raw: &RawValue, limit: usize) -> mlua::Result<Cow<'_, str>> {
    let source = raw.get();
    if source.len() > limit.saturating_mul(6).saturating_add(2) {
        return Err(mlua::Error::runtime(
            "binary JSON string exceeds size limit",
        ));
    }
    let value = if !source.as_bytes().contains(&b'\\') {
        Cow::Borrowed(&source[1..source.len() - 1])
    } else {
        Cow::Owned(serde_json::from_str::<String>(source).map_err(super::super::system::error)?)
    };
    if value.len() > limit {
        return Err(mlua::Error::runtime(
            "binary JSON string exceeds size limit",
        ));
    }
    Ok(value)
}
