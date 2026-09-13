use super::{Mode, Options, MAX_INPUT_BYTES, MAX_RENDERED_BYTES};
use crate::native::Budget;
use anyhow::{ensure, Result};
use serde::Serialize;
use std::{borrow::Cow, io};

/// 【文档转换】【有界结果】返回正文和完整字符计数，业务提示由调用插件自行组合
#[derive(Serialize)]
pub(crate) struct Document<T = String> {
    text: T,
    total_chars: usize,
    truncated: bool,
}

/// 【文档转换】【原生计算】解码完整 UTF-8 替换文本，按指定格式转换后进行字符摘录
/// @param bytes 原始字节；options 为已校验选项；budget 为共享预算；output_bytes 为完整结果上限
/// @returns 完整字符计数和有界前缀，不读取文件、网络或业务配置
pub(crate) fn render(
    bytes: &[u8],
    options: Options,
    budget: Budget,
    output_bytes: usize,
) -> Result<Document> {
    ensure!(
        bytes.len() <= MAX_INPUT_BYTES,
        "document input exceeds size limit"
    );
    options.validate()?;
    // 1. 【文档转换】【完整解码】不使用 Content-Type 的 charset，保留原始替换解码语义
    budget(bytes.len().max(1) as u64)?;
    let decoded = String::from_utf8_lossy(bytes);
    budget(0)?;
    // 2. 【文档转换】【格式计算】转换库内部不支持取消，只在进入和返回边界检查期限
    let output = match options.mode {
        Mode::Raw => Cow::Borrowed(decoded.as_ref()),
        Mode::HtmlText | Mode::HtmlMarkdown => {
            budget(bytes.len().max(1) as u64)?;
            let output = match options.mode {
                // 1. 【文档转换】【长词换行】使用按偏移前进的转换器，避免重复扫描和复制剩余长词
                Mode::HtmlText => {
                    html2text_document::from_read(decoded.as_bytes(), options.width.unwrap_or(120))?
                }
                Mode::HtmlMarkdown => html2md::parse_html(&decoded),
                Mode::Raw => unreachable!(),
            };
            budget(0)?;
            ensure!(
                output.len() <= MAX_RENDERED_BYTES,
                "document intermediate output exceeds size limit"
            );
            Cow::Owned(output)
        }
    };
    // 3. 【文档转换】【全文计数】只保存前缀边界，按字符数而不是 UTF-8 字节裁剪
    let limit = options.max_chars.unwrap_or(24_000);
    let mut total_chars = 0;
    let mut end = output.len();
    for (offset, _) in output.char_indices() {
        if total_chars == limit {
            end = offset;
        }
        total_chars += 1;
        if total_chars % 4096 == 0 {
            budget(4096)?;
        }
    }
    budget((total_chars % 4096) as u64)?;
    let text = &output[..end];
    let truncated = total_chars > limit;
    // 4. 【文档转换】【交付大小】无分配地计量完整 JSON，包含所有控制字符转义和元数据
    let view = Document {
        text,
        total_chars,
        truncated,
    };
    serde_json::to_writer(SizeLimit(output_bytes), &view)
        .map_err(|_| anyhow::anyhow!("document result exceeds output limit"))?;
    budget(0)?;
    Ok(Document {
        text: text.to_string(),
        total_chars,
        truncated,
    })
}

struct SizeLimit(usize);

impl io::Write for SizeLimit {
    /// 【文档转换】【字节计量】只计算序列化字节，不为结果额外分配大缓冲
    /// @param bytes 序列化器产生的下一段字节
    /// @returns 计量字节数，超出总额度时中止
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self
            .0
            .checked_sub(bytes.len())
            .ok_or_else(|| io::Error::other("document output limit"))?;
        Ok(bytes.len())
    }

    /// 【文档转换】【计量完成】计量器没有需要刷新到外部的内容
    /// @returns 始终成功
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
