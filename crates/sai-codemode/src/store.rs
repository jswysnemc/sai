use anyhow::{bail, Result};
use serde_json::Value;
use std::collections::BTreeMap;

pub type Store = BTreeMap<String, Value>;
pub const MAX_VALUE_BYTES: usize = 256 * 1024;
pub const MAX_STORE_BYTES: usize = 1024 * 1024;

/// 【CodeMode】【状态校验】校验 values 的键和值容量，返回校验结果。
pub fn validate(values: &Store) -> Result<()> {
    for (key, value) in values {
        if key.len() > 1024 || serde_json::to_vec(value)?.len() > MAX_VALUE_BYTES {
            bail!("codemode store value exceeds size limit");
        }
    }
    if serde_json::to_vec(values)?.len() > MAX_STORE_BYTES {
        bail!("codemode store exceeds total size limit");
    }
    Ok(())
}

/// 【CodeMode】【状态合并】把 writes 应用到 values，返回新快照；null 增量表示删除。
pub fn apply(values: &Store, writes: &Store) -> Result<Store> {
    let mut result = values.clone();
    for (key, value) in writes {
        if value.is_null() {
            result.remove(key);
        } else {
            // 增量包装可区分保存 null 与删除键
            let Some(value) = value.get("value") else {
                bail!("invalid codemode store write");
            };
            result.insert(key.clone(), value.clone());
        }
    }
    validate(&result)?;
    Ok(result)
}
