use super::*;

/// 【服务商配置】【目录回归】过期请求只进入缓存，不能覆盖当前供应商。
/// 参数: 无；返回: 无
#[test]
fn delayed_result_is_isolated_and_reused() {
    let mut config = AppConfig::default();
    config.providers.truncate(2);
    let mut browser = ProviderBrowser::new(&mut config);
    let first_key = browser.catalog_key();
    let (first_tx, first_rx) = mpsc::channel();
    browser.pending_fetches.insert(first_key.clone(), first_rx);
    browser.provider_idx = 1;
    let second_key = browser.catalog_key();
    let (_second_tx, second_rx) = mpsc::channel();
    browser.pending_fetches.insert(second_key, second_rx);
    browser.refresh_models();
    first_tx
        .send(Ok(FetchModelsResult {
            models: vec!["old-remote".into()],
            metadata: BTreeMap::new(),
        }))
        .unwrap();
    browser.poll_fetch_result();
    assert!(!browser.raw_models.iter().any(|model| model == "old-remote"));
    assert!(browser.loading);
    browser.provider_idx = 0;
    browser.refresh_models();
    assert!(browser.raw_models.iter().any(|model| model == "old-remote"));
    assert!(!browser.loading);
    assert_eq!(browser.pending_fetches.len(), 1);
}

/// 【服务商配置】【目录回归】重复切换复用请求，失败缓存直到显式刷新。
/// 参数: 无；返回: 无
#[test]
fn repeated_refresh_deduplicates_and_keeps_local_models() {
    let mut config = AppConfig::default();
    config.providers[0].models = vec!["local".into()];
    let mut browser = ProviderBrowser::new(&mut config);
    let key = browser.catalog_key();
    let (sender, receiver) = mpsc::channel();
    browser.pending_fetches.insert(key.clone(), receiver);
    for _ in 0..10 {
        browser.refresh_models();
    }
    assert_eq!(browser.pending_fetches.len(), 1);
    assert!(browser.raw_models.contains(&"local".to_string()));
    sender.send(Err("fixture failure".into())).unwrap();
    browser.poll_fetch_result();
    browser.refresh_models();
    assert!(!browser.loading);
    assert!(browser.pending_fetches.is_empty());
    assert!(browser.status.contains("fixture failure"));
    browser.config.providers[0].api_key = Some("changed".into());
    assert_ne!(browser.catalog_key(), key);
}

/// 【服务商配置】【并发回归】已有四个请求时仍允许立即导航，但不再创建线程。
/// 参数: 无；返回: 无
#[test]
fn saturated_requests_do_not_block_navigation() {
    let mut config = AppConfig::default();
    let mut browser = ProviderBrowser::new(&mut config);
    let mut senders = Vec::new();
    for index in 0..4 {
        let (sender, receiver) = mpsc::channel();
        senders.push(sender);
        browser.pending_fetches.insert(index.to_string(), receiver);
    }
    browser.refresh_models();
    let signature = browser.frame_signature();
    browser.move_right();
    assert_ne!(browser.frame_signature(), signature);
    browser.move_down();
    assert_eq!(browser.pending_fetches.len(), 4);
}
