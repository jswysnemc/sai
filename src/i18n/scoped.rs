use super::Locale;
use std::cell::Cell;

thread_local! {
    static SCOPED_LOCALE: Cell<Option<Locale>> = const { Cell::new(None) };
}

struct Restore(Option<Locale>);

impl Drop for Restore {
    /// 【语言上下文】【恢复作用域】无参数；正常返回和异常展开都恢复当前线程原有语言
    fn drop(&mut self) {
        SCOPED_LOCALE.set(self.0);
    }
}

/// 【语言上下文】【当前作用域】无参数；返回当前同步调用指定的语言，未指定时返回空
pub(super) fn current() -> Option<Locale> {
    SCOPED_LOCALE.get()
}

/// 【语言上下文】【同步覆盖】临时使用任务语言，不修改进程设置或其他线程
/// @param language 本次语言；callback 为同步操作，不得让异步任务依赖此线程作用域
/// @returns 回调结果，退出后恢复原语言
pub(crate) fn with_locale<T>(language: Locale, callback: impl FnOnce() -> T) -> T {
    let _restore = Restore(SCOPED_LOCALE.replace(Some(language)));
    callback()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【语言上下文测试】【嵌套恢复】无参数；内层异常和外层结束后分别恢复原有语言
    #[test]
    fn scoped_locale_restores_nested_context_after_panics() {
        let original = crate::i18n::locale();
        with_locale(Locale::En, || {
            assert_eq!(crate::i18n::locale(), Locale::En);
            let result = std::panic::catch_unwind(|| {
                with_locale(Locale::Zh, || {
                    assert_eq!(crate::i18n::locale(), Locale::Zh);
                    panic!("locale fixture");
                });
            });
            assert!(result.is_err());
            assert_eq!(crate::i18n::locale(), Locale::En);
        });
        assert_eq!(crate::i18n::locale(), original);
    }

    /// 【语言上下文测试】【线程隔离】无参数；并发同步操作不会更改彼此的语言
    #[test]
    fn scoped_locales_are_isolated_between_threads() {
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let child_barrier = barrier.clone();
        with_locale(Locale::En, || {
            let child = std::thread::spawn(move || {
                with_locale(Locale::Zh, || {
                    child_barrier.wait();
                    assert_eq!(crate::i18n::locale(), Locale::Zh);
                });
            });
            barrier.wait();
            assert_eq!(crate::i18n::locale(), Locale::En);
            child.join().unwrap();
        });
    }
}
