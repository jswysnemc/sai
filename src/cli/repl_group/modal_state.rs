//! 群组工作台弹窗状态定义：统一使用字符下标管理表单光标与字段切换。

/// 终端群组居中弹窗类型（所有字段的 `cursor` 均为字符下标）。
#[derive(Debug, Clone)]
pub(super) enum GroupModal {
    CreateGroup {
        name: String,
        description: String,
        field: usize,
        cursor: usize,
    },
    SpawnRunner {
        name: String,
        specialty: String,
        prompt: String,
        field: usize,
        cursor: usize,
    },
    BindChannel {
        channel: String,
        chat_id: String,
        admission: String,
        field: usize,
        cursor: usize,
    },
    ChannelSim {
        channel: String,
        chat_id: String,
        actor: String,
        content: String,
        field: usize,
        cursor: usize,
    },
    Help,
}

impl GroupModal {
    /// 返回弹窗标题。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 弹窗标题文本
    pub(super) fn title(&self) -> &'static str {
        match self {
            Self::CreateGroup { .. } => "新建协作群组",
            Self::SpawnRunner { .. } => "启动异步 Runner",
            Self::BindChannel { .. } => "绑定外部渠道（微信 / 飞书 / QQ / OneBot）",
            Self::ChannelSim { .. } => "本地模拟外部渠道入站",
            Self::Help => "群组工作台快捷键与架构说明",
        }
    }

    /// 返回当前弹窗可编辑字段数。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 可编辑字段数量
    pub(super) fn field_count(&self) -> usize {
        match self {
            Self::CreateGroup { .. } => 2,
            Self::SpawnRunner { .. } | Self::BindChannel { .. } => 3,
            Self::ChannelSim { .. } => 4,
            Self::Help => 0,
        }
    }

    /// 切换下一个或上一个输入字段，并将字符光标同步到新字段末尾。
    ///
    /// 参数:
    /// - `delta`: +1 下一个字段，-1 上一个字段
    ///
    /// 返回:
    /// - 无
    pub(super) fn step_field(&mut self, delta: isize) {
        let count = self.field_count();
        if count == 0 {
            return;
        }
        match self {
            Self::CreateGroup { field, .. }
            | Self::SpawnRunner { field, .. }
            | Self::BindChannel { field, .. }
            | Self::ChannelSim { field, .. } => {
                *field = (*field as isize + delta).rem_euclid(count as isize) as usize;
            }
            Self::Help => return,
        }
        let char_len = self.active_text().chars().count();
        if let Some(cursor) = self.cursor_mut() {
            *cursor = char_len;
        }
    }

    /// 读取当前激活字段的只读引用。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 当前字段文本
    pub(super) fn active_text(&self) -> &str {
        match self {
            Self::CreateGroup {
                name,
                description,
                field,
                ..
            } => {
                if *field == 0 {
                    name
                } else {
                    description
                }
            }
            Self::SpawnRunner {
                name,
                specialty,
                prompt,
                field,
                ..
            } => match *field {
                0 => name,
                1 => specialty,
                _ => prompt,
            },
            Self::BindChannel {
                channel,
                chat_id,
                admission,
                field,
                ..
            } => match *field {
                0 => channel,
                1 => chat_id,
                _ => admission,
            },
            Self::ChannelSim {
                channel,
                chat_id,
                actor,
                content,
                field,
                ..
            } => match *field {
                0 => channel,
                1 => chat_id,
                2 => actor,
                _ => content,
            },
            Self::Help => "",
        }
    }

    /// 读取当前激活字段文本与字符光标的可变引用。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - `(字段文本, 字符光标)`
    pub(super) fn active_field_mut(&mut self) -> Option<(&mut String, &mut usize)> {
        match self {
            Self::CreateGroup {
                name,
                description,
                field,
                cursor,
            } => Some((if *field == 0 { name } else { description }, cursor)),
            Self::SpawnRunner {
                name,
                specialty,
                prompt,
                field,
                cursor,
            } => Some((
                match *field {
                    0 => name,
                    1 => specialty,
                    _ => prompt,
                },
                cursor,
            )),
            Self::BindChannel {
                channel,
                chat_id,
                admission,
                field,
                cursor,
            } => Some((
                match *field {
                    0 => channel,
                    1 => chat_id,
                    _ => admission,
                },
                cursor,
            )),
            Self::ChannelSim {
                channel,
                chat_id,
                actor,
                content,
                field,
                cursor,
            } => Some((
                match *field {
                    0 => channel,
                    1 => chat_id,
                    2 => actor,
                    _ => content,
                },
                cursor,
            )),
            Self::Help => None,
        }
    }

    /// 读取当前字段字符光标可变引用。
    fn cursor_mut(&mut self) -> Option<&mut usize> {
        match self {
            Self::CreateGroup { cursor, .. }
            | Self::SpawnRunner { cursor, .. }
            | Self::BindChannel { cursor, .. }
            | Self::ChannelSim { cursor, .. } => Some(cursor),
            Self::Help => None,
        }
    }
}
