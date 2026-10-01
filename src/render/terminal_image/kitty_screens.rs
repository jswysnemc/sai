/// 可在切屏后补传的图片数据上限；超出后最早登记的图片不再补传。
const KITTY_REPLAY_LIMIT: usize = 256;

/// 【终端图片】【分屏传输】Kitty 协议的主屏与备用屏各有一份图片存储。
///
/// 图片数据按 image_id 只传一次，后续只打放置序列；进入 Ctrl+O 全屏（备用屏）
/// 后，主屏传过的图片在备用屏里不存在，放置序列引用不到数据，公式整片空白。
/// 这里记录每块屏幕已传输的 image_id，并保留数据以便在另一块屏幕按需补传。
#[derive(Default)]
struct KittyScreens {
    /// 当前是否处于备用屏
    alternate: bool,
    /// 主屏已传输的图片
    main_sent: std::collections::HashSet<u32>,
    /// 备用屏已传输的图片
    alternate_sent: std::collections::HashSet<u32>,
    /// 可补传的传输载荷，按登记顺序淘汰
    payloads: std::collections::HashMap<u32, String>,
    order: std::collections::VecDeque<u32>,
}

impl KittyScreens {
    /// 返回当前屏幕的已传输集合。
    fn sent_mut(&mut self) -> &mut std::collections::HashSet<u32> {
        if self.alternate {
            &mut self.alternate_sent
        } else {
            &mut self.main_sent
        }
    }

    /// 登记图片数据；当前屏幕首次见到时返回 true，调用方需立即传输。
    ///
    /// 参数:
    /// - `image_id`: 图片标识
    /// - `payload`: 传输载荷
    ///
    /// 返回:
    /// - 是否需要在当前屏幕传输
    fn register(&mut self, image_id: u32, payload: &str) -> bool {
        if let std::collections::hash_map::Entry::Vacant(entry) = self.payloads.entry(image_id) {
            entry.insert(payload.to_string());
            self.order.push_back(image_id);
            while self.order.len() > KITTY_REPLAY_LIMIT {
                if let Some(old) = self.order.pop_front() {
                    self.payloads.remove(&old);
                }
            }
        }
        self.sent_mut().insert(image_id)
    }

    /// 返回文本中引用、但当前屏幕尚未传输的图片载荷，并标记为已传输。
    ///
    /// 参数:
    /// - `text`: 即将写入终端的文本
    ///
    /// 返回:
    /// - 需先于文本写出的传输载荷
    fn missing_for(&mut self, text: &str) -> String {
        let mut output = String::new();
        for image_id in placement_image_ids(text) {
            let Some(payload) = self.payloads.get(&image_id).cloned() else {
                continue;
            };
            if self.sent_mut().insert(image_id) {
                output.push_str(&payload);
            }
        }
        output
    }
}

/// 返回进程级分屏传输状态。
fn kitty_screens() -> &'static std::sync::Mutex<KittyScreens> {
    static STATE: std::sync::OnceLock<std::sync::Mutex<KittyScreens>> =
        std::sync::OnceLock::new();
    STATE.get_or_init(|| std::sync::Mutex::new(KittyScreens::default()))
}

/// 【终端图片】【切屏】切换当前屏幕；进入备用屏时清空其传输记录。
///
/// 备用屏每次进入都是新的图片存储，上次全屏传过的数据已随退出释放。
///
/// 参数:
/// - `alternate`: 是否进入备用屏
///
/// 返回:
/// - 无
pub(crate) fn set_kitty_alternate_screen(alternate: bool) {
    if let Ok(mut state) = kitty_screens().lock() {
        state.alternate = alternate;
        if alternate {
            state.alternate_sent.clear();
        }
    }
}

/// 【终端图片】【整屏删除】返回删除全部放置的序列，并清空当前屏幕的传输记录。
///
/// 整屏重绘前会删除全部放置；Kitty 会顺带回收不再有放置的图片数据，
/// 之后只发放置序列会引用到已释放的数据，缩放终端后公式整片消失。
/// 清空记录后，下一帧提交时按引用重新补传。
///
/// 返回:
/// - 删除全部放置的转义序列
pub(crate) fn kitty_delete_placements() -> &'static str {
    if let Ok(mut state) = kitty_screens().lock() {
        state.sent_mut().clear();
    }
    KITTY_DELETE_PLACEMENTS
}

/// 【终端图片】【按需补传】返回即将写出文本所引用、当前屏幕缺失的图片数据。
///
/// 参数:
/// - `text`: 即将写入终端的文本
///
/// 返回:
/// - 应先于文本写出的传输载荷；无缺失时为空
pub(crate) fn kitty_missing_transmissions(text: &str) -> String {
    if !text.contains("\x1b_Ga=p") {
        return String::new();
    }
    kitty_screens()
        .lock()
        .map(|mut state| state.missing_for(text))
        .unwrap_or_default()
}

/// 【终端图片】【格高】返回块级图片补齐网格时使用的单格像素高度。
///
/// 返回:
/// - 经等宽字体比例校正的格高
pub(crate) fn kitty_cell_pixel_height() -> usize {
    let (cell_pw, cell_ph) = terminal_cell_pixel_size();
    normalize_mono_cell_pixels(cell_pw, cell_ph).1
}

/// 登记图片数据并判断当前屏幕是否需要传输。
///
/// 参数:
/// - `image_id`: 图片标识
/// - `payload`: 传输载荷
///
/// 返回:
/// - 当前屏幕首次见到该图片时返回 true
fn register_kitty_payload(image_id: u32, payload: &str) -> bool {
    kitty_screens()
        .lock()
        .map(|mut state| state.register(image_id, payload))
        .unwrap_or(true)
}

/// 提取文本中所有 Kitty 放置序列引用的 image_id。
///
/// 参数:
/// - `text`: 终端文本
///
/// 返回:
/// - 去重后的 image_id，保持出现顺序
fn placement_image_ids(text: &str) -> Vec<u32> {
    let mut ids = Vec::new();
    for chunk in text.split("\x1b_G").skip(1) {
        let control = chunk.split(['\x1b', ';']).next().unwrap_or_default();
        if !control.split(',').any(|part| part == "a=p") {
            continue;
        }
        let id = control
            .split(',')
            .find_map(|part| part.strip_prefix("i="))
            .and_then(|value| value.parse::<u32>().ok());
        if let Some(id) = id.filter(|id| !ids.contains(id)) {
            ids.push(id);
        }
    }
    ids
}

#[cfg(test)]
mod kitty_screen_tests {
    use super::*;

    /// 验证只提取放置序列的 image_id，传输序列被忽略。
    #[test]
    fn extracts_placement_ids_only() {
        let text = "\x1b_Ga=t,f=100,q=2,i=9,m=0;AAAA\x1b\\x \x1b_Ga=p,q=2,C=1,i=7,p=1,c=3,r=1\x1b\\ y \x1b_Ga=p,q=2,C=1,i=7,p=2\x1b\\";
        assert_eq!(placement_image_ids(text), vec![7]);
    }

    /// 验证进入备用屏后缺失的图片被补传一次，回到主屏不重复传输。
    #[test]
    fn alternate_screen_replays_missing_images_once() {
        let mut screens = KittyScreens::default();
        assert!(screens.register(7, "<data-7>"));
        assert!(!screens.register(7, "<data-7>"));
        let frame = "\x1b_Ga=p,q=2,C=1,i=7,p=1\x1b\\";
        assert!(screens.missing_for(frame).is_empty());

        screens.alternate = true;
        screens.alternate_sent.clear();
        assert_eq!(screens.missing_for(frame), "<data-7>");
        assert!(screens.missing_for(frame).is_empty());

        screens.alternate = false;
        assert!(screens.missing_for(frame).is_empty());
    }

    /// 验证全屏期间新渲染的图片回到主屏后仍会补传。
    #[test]
    fn images_first_seen_in_fullscreen_replay_on_main_screen() {
        let mut screens = KittyScreens {
            alternate: true,
            ..KittyScreens::default()
        };
        assert!(screens.register(11, "<data-11>"));
        screens.alternate = false;
        assert_eq!(
            screens.missing_for("\x1b_Ga=p,q=2,C=1,i=11,p=4\x1b\\"),
            "<data-11>"
        );
    }

    /// 验证整屏删除后，同一屏幕再次引用的图片会重新补传。
    #[test]
    fn full_delete_forces_retransmission() {
        let mut screens = KittyScreens::default();
        assert!(screens.register(21, "<data-21>"));
        let frame = "\x1b_Ga=p,q=2,C=1,i=21,p=9\x1b\\";
        assert!(screens.missing_for(frame).is_empty());
        screens.sent_mut().clear();
        assert_eq!(screens.missing_for(frame), "<data-21>");
    }

    /// 验证超出上限时淘汰最早登记的载荷。
    #[test]
    fn replay_store_is_bounded() {
        let mut screens = KittyScreens::default();
        for id in 0..(KITTY_REPLAY_LIMIT as u32 + 5) {
            screens.register(id + 1, "x");
        }
        assert_eq!(screens.payloads.len(), KITTY_REPLAY_LIMIT);
        assert!(!screens.payloads.contains_key(&1));
    }
}
