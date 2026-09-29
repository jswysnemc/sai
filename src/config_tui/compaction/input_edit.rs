use super::draft::Draft;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl Draft {
    /// 【上下文】【数值编辑】打开输入并全选原值，直接输入即可替换。
    /// 参数: value 为已有数值；返回: 无
    pub(super) fn begin_input(&mut self, value: String) {
        self.input_cursor = value.len();
        self.input_selected = true;
        self.input = Some(value);
    }

    /// 【上下文】【数值编辑】处理组合键和普通按键。
    /// 参数: key 为键盘事件；返回: 保存或取消操作
    pub(super) fn handle_event(&mut self, key: KeyEvent) -> Option<super::draft::Outcome> {
        if self.input.is_some() && key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('a') => self.input_selected = true,
                KeyCode::Char('u') => {
                    self.input = Some(String::new());
                    self.input_cursor = 0;
                    self.input_selected = false;
                }
                _ => {}
            }
            return None;
        }
        self.handle(key.code)
    }

    /// 【上下文】【数值编辑】编辑 ASCII 数值，支持插入、删除、首尾移动和撤销。
    /// 参数: key 为按键；返回: 无，非法数值保留在草稿中
    pub(super) fn edit_input(&mut self, key: KeyCode) {
        let value = self.input.as_mut().unwrap();
        self.input_cursor = self.input_cursor.min(value.len());
        match key {
            KeyCode::Esc => self.input = None,
            KeyCode::Enter | KeyCode::Tab => match self.preview() {
                Ok(policy) => {
                    self.policy = policy;
                    self.input = None;
                    self.reset = false;
                    if key == KeyCode::Tab {
                        self.selected = (self.selected + 1) % 5;
                    }
                }
                Err(error) => self.error = Some(error.to_string()),
            },
            KeyCode::Home => {
                self.input_cursor = 0;
                self.input_selected = false;
            }
            KeyCode::End => {
                self.input_cursor = value.len();
                self.input_selected = false;
            }
            KeyCode::Left => {
                self.input_cursor = if self.input_selected {
                    0
                } else {
                    self.input_cursor.saturating_sub(1)
                };
                self.input_selected = false;
            }
            KeyCode::Right => {
                self.input_cursor = (self.input_cursor + 1).min(value.len());
                self.input_selected = false;
            }
            KeyCode::Backspace | KeyCode::Delete => {
                if self.input_selected {
                    value.clear();
                    self.input_cursor = 0;
                } else if key == KeyCode::Backspace && self.input_cursor > 0 {
                    self.input_cursor -= 1;
                    value.remove(self.input_cursor);
                } else if key == KeyCode::Delete && self.input_cursor < value.len() {
                    value.remove(self.input_cursor);
                }
                self.input_selected = false;
            }
            KeyCode::Char(ch) if ch.is_ascii_graphic() && value.len() < 32 => {
                if self.input_selected {
                    value.clear();
                    self.input_cursor = 0;
                    self.input_selected = false;
                }
                value.insert(self.input_cursor, ch);
                self.input_cursor += 1;
            }
            _ => {}
        }
    }

    /// 【上下文】【输入展示】显示选区或插入光标，保持数值可辨认。
    /// 参数: 无；返回: 可直接绘制的数值
    pub(super) fn input_display(&self) -> String {
        let value = self.input.as_deref().unwrap_or_default();
        if self.input_selected {
            return format!("[{value}]");
        }
        let cursor = self.input_cursor.min(value.len());
        format!("{}|{}", &value[..cursor], &value[cursor..])
    }
}
