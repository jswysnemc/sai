use crate::protocol::OutputKind;
use anyhow::{bail, Result};

pub const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_OUTPUT_ITEMS: usize = 100_000;

#[derive(Default)]
pub struct Output {
    pub items: Vec<(OutputKind, String)>,
    bytes: usize,
}

impl Output {
    /// 【CodeMode】【输出收集】添加 kind/value 并校验总容量，返回收集结果。
    pub fn push(&mut self, kind: OutputKind, value: String) -> Result<()> {
        if self.bytes.saturating_add(value.len()) > MAX_OUTPUT_BYTES
            || self.items.len() >= MAX_OUTPUT_ITEMS
        {
            bail!("codemode output exceeds size limit");
        }
        self.bytes += value.len();
        self.items.push((kind, value));
        Ok(())
    }
}
