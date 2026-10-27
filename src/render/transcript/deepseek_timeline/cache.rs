use super::node::RenderedNode;

/// 【终端】【时间线缓存条目】单个历史节点在特定排版宽度和折叠状态下的渲染产物。
#[derive(Clone)]
struct CachedTimelineNode {
    width: usize,
    open: bool,
    node: RenderedNode,
}

/// 【终端】【时间线节点缓存】避免在流式生成或鼠标悬停重绘时对定稿步骤重复反序列化与换行。
#[derive(Default)]
pub(in crate::render::transcript) struct TimelineNodeCache {
    entries: Vec<Option<CachedTimelineNode>>,
}

impl TimelineNodeCache {
    /// 【终端】【缓存槽位对齐】随新 cell 存入扩充空槽位。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_slot(&mut self) {
        self.entries.push(None);
    }

    /// 【终端】【单项失效】特定单元内容更新时作废缓存。
    ///
    /// 参数:
    /// - `index`: 节点下标
    ///
    /// 返回:
    /// - 无
    pub(crate) fn invalidate(&mut self, index: usize) {
        if let Some(entry) = self.entries.get_mut(index) {
            *entry = None;
        }
    }

    /// 【终端】【全量清理】清屏或切换样式时重置全部缓存。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    /// 【终端】【读取或渲染】若命中宽度与展开状态则直接复用，否则调用生成器并缓存。
    ///
    /// 参数:
    /// - `index`: 单元索引
    /// - `width`: 净宽
    /// - `open`: 展开状态
    /// - `render`: 渲染闭包
    ///
    /// 返回:
    /// - 渲染后的节点数据
    pub(super) fn get_or_render(
        &mut self,
        index: usize,
        width: usize,
        open: bool,
        render: impl FnOnce() -> RenderedNode,
    ) -> RenderedNode {
        // 1. 【终端】【缓存命中】检查当前槽位宽度与展开状态
        if let Some(Some(cached)) = self.entries.get(index) {
            if cached.width == width && cached.open == open {
                return cached.node.clone();
            }
        }

        // 2. 【终端】【未命中计算】执行实际排版渲染
        let node = render();

        // 3. 【终端】【写回缓存】自动扩充槽位容量并保存渲染产物
        if index >= self.entries.len() {
            self.entries.resize(index + 1, None);
        }
        self.entries[index] = Some(CachedTimelineNode {
            width,
            open,
            node: node.clone(),
        });

        node
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::transcript::AnsiLine;

    #[test]
    fn test_timeline_node_cache_hit_and_invalidate() {
        let mut cache = TimelineNodeCache::default();
        let mut render_count = 0;

        let node1 = cache.get_or_render(0, 80, true, || {
            render_count += 1;
            RenderedNode {
                lines: vec![AnsiLine::new("Title".into())],
                open: true,
            }
        });
        assert_eq!(render_count, 1);
        assert_eq!(node1.lines.len(), 1);

        // 命中同一宽度和状态，不再调用渲染器
        let node2 = cache.get_or_render(0, 80, true, || {
            render_count += 1;
            panic!("Should not be called");
        });
        assert_eq!(render_count, 1);
        assert_eq!(node2.lines.len(), 1);

        // 宽度改变，重新渲染
        let _ = cache.get_or_render(0, 100, true, || {
            render_count += 1;
            RenderedNode {
                lines: vec![AnsiLine::new("Wide".into())],
                open: true,
            }
        });
        assert_eq!(render_count, 2);

        // 作废后重新渲染
        cache.invalidate(0);
        let _ = cache.get_or_render(0, 100, true, || {
            render_count += 1;
            RenderedNode {
                lines: vec![AnsiLine::new("After Invalidate".into())],
                open: true,
            }
        });
        assert_eq!(render_count, 3);
    }
}
