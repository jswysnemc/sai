use crate::render::input_atom::{InputAtom, InputAtomKind};
use crate::render::omitted_line::{render_fold_hint, FOLD_SHORTCUT};

/// 【终端】【用户图片】识别具有真实来源的图片附件，不把普通标签当作图片。
/// 参数: `atoms` 为用户消息附件；返回: 是否存在可预览图片。
pub(super) fn has_images(atoms: &[InputAtom]) -> bool {
    atoms
        .iter()
        .any(|atom| atom.kind == InputAtomKind::Image && atom.image_source.is_some())
}

/// 【终端】【附件预览】默认仅展示折叠提示，展开时逐张显示左对齐缩略图。
/// 参数: `atoms` 为附件，`expanded` 为展开状态，`width` 为消息净宽；返回: 追加到消息的物理行。
pub(super) fn lines(atoms: &[InputAtom], expanded: bool, width: usize) -> Vec<String> {
    let images: Vec<_> = atoms
        .iter()
        .filter(|atom| atom.kind == InputAtomKind::Image)
        .filter_map(|atom| atom.image_source.as_deref())
        .collect();
    if images.is_empty() {
        return Vec::new();
    }
    if !expanded {
        let unit = if images.len() == 1 {
            crate::render::terminal_text("image", "张图片")
        } else {
            crate::render::terminal_text("images", "张图片")
        };
        return vec![format!(
            "  {}",
            render_fold_hint(&format!("{} {unit}", images.len()), Some(FOLD_SHORTCUT))
        )];
    }
    images
        .into_iter()
        .flat_map(|source| {
            crate::render::image_preview::render(source, width)
                .split('\n')
                .map(|line| format!("  {line}"))
                .collect::<Vec<_>>()
        })
        .collect()
}
