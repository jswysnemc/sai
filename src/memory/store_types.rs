/// 从上下文中淘汰的单轮对话。
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct EvictedTurn {
    pub timestamp: String,
    pub role: String,
    pub content: String,
}

/// 联想检索返回的事实与经历上下文。
#[derive(Debug, Clone)]
pub struct AssociationContext {
    pub facts: Vec<MemoryHit>,
    pub episodes: Vec<MemoryHit>,
}

/// 单条记忆检索命中。
#[derive(Debug, Clone)]
pub struct MemoryHit {
    pub id: i64,
    pub content: String,
    pub score: f32,
    pub timestamp: String,
    pub source: String,
    pub tags: Vec<String>,
}
