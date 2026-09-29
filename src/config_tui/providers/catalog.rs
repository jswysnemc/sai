use super::*;

impl ProviderBrowser<'_> {
    /// 【服务商配置】【目录缓存】生成包含接入配置的缓存键，仅存于内存且不输出日志。
    /// @returns 当前供应商配置指纹，避免修改密钥或地址后复用旧结果
    pub(super) fn catalog_key(&self) -> String {
        self.config
            .providers
            .get(self.provider_idx)
            .and_then(|provider| serde_json::to_string(provider).ok())
            .map(|json| blake3::hash(json.as_bytes()).to_hex().to_string())
            .unwrap_or_default()
    }

    /// 【服务商配置】【目录切换】立即显示本地模型，并复用相同配置的缓存或在途请求。
    /// @returns 无；网络请求始终在后台执行
    pub(super) fn refresh_models(&mut self) {
        self.provider_idx = self
            .provider_idx
            .min(self.config.providers.len().saturating_sub(1));
        self.raw_models = self
            .config
            .providers
            .get(self.provider_idx)
            .map(local_provider_models)
            .unwrap_or_default();
        self.remote_metadata.clear();
        self.org_idx = 0;
        self.model_idx = 0;
        self.rebuild_models();
        self.loading = !self.config.providers.is_empty();
        self.status = if self.loading {
            t("Fetching model list...", "正在获取模型列表...").into()
        } else {
            String::new()
        };
        self.poll_fetch_result();
    }

    /// 【服务商配置】【异步目录】收集完成的请求，只将当前供应商结果应用到画面。
    /// @returns 无；最多四个并发请求，切换或按刷新不重复启动相同配置请求
    pub(super) fn poll_fetch_result(&mut self) {
        let completed = self
            .pending_fetches
            .iter()
            .filter_map(|(key, receiver)| match receiver.try_recv() {
                Ok(result) => Some((key.clone(), result)),
                Err(mpsc::TryRecvError::Disconnected) => Some((
                    key.clone(),
                    Err(t("Model request stopped", "模型请求已中断").into()),
                )),
                Err(mpsc::TryRecvError::Empty) => None,
            })
            .collect::<Vec<_>>();
        for (key, result) in completed {
            self.pending_fetches.remove(&key);
            self.model_cache.insert(key, result);
        }
        if !self.loading {
            return;
        }
        let key = self.catalog_key();
        if let Some(result) = self.model_cache.get(&key).cloned() {
            self.loading = false;
            match result {
                Ok(result) => {
                    self.status = format!(
                        "{} {} · r {}",
                        result.models.len(),
                        t("models", "个模型"),
                        t("refresh", "刷新")
                    );
                    self.raw_models.extend(result.models);
                    self.remote_metadata = result.metadata;
                    self.rebuild_models();
                }
                Err(error) => {
                    self.status = format_status_line(&format!(
                        "{}: {error}",
                        t("Failed to fetch models", "获取模型失败")
                    ))
                }
            }
        } else if !self.pending_fetches.contains_key(&key) && self.pending_fetches.len() < 4 {
            if let Some(provider) = self.config.providers.get(self.provider_idx).cloned() {
                let (sender, receiver) = mpsc::channel();
                self.pending_fetches.insert(key, receiver);
                std::thread::spawn(move || {
                    let result = fetch_models(&provider).map_err(|error| error.to_string());
                    let _ = sender.send(result);
                });
            }
        }
    }

    /// 按过滤词和组织重建模型候选，无参数，返回值为空。
    pub(super) fn rebuild_models(&mut self) {
        let mut grouped: BTreeMap<String, Vec<ModelEntry>> = BTreeMap::new();
        let ordered_models = self.prioritized_models();
        for model in &ordered_models {
            if !model_matches_filter(model, &self.filter) {
                continue;
            }
            let org = model
                .split_once('/')
                .map(|(org, _)| org)
                .unwrap_or("All")
                .to_string();
            let name = model
                .split_once('/')
                .map(|(_, name)| name)
                .unwrap_or(model)
                .to_string();
            grouped
                .entry("All".to_string())
                .or_default()
                .push(ModelEntry::new(model, model));
            if org != "All" {
                grouped
                    .entry(org)
                    .or_default()
                    .push(ModelEntry::new(&name, model));
            }
        }
        self.orgs = grouped.keys().cloned().collect();
        if self.orgs.is_empty() {
            self.orgs.push("All".to_string());
        }
        self.org_idx = super::super::search::clamp_index(self.org_idx, self.orgs.len());
        self.models = grouped.remove(&self.orgs[self.org_idx]).unwrap_or_default();
        self.model_idx = super::super::search::clamp_index(self.model_idx, self.models.len());
    }

    /// 返回本地已激活模型优先的合并列表。
    pub(super) fn prioritized_models(&self) -> Vec<String> {
        let mut models = self
            .config
            .providers
            .get(self.provider_idx)
            .map(local_provider_models)
            .unwrap_or_default();
        for model in &self.raw_models {
            if !models.iter().any(|item| item == model) {
                models.push(model.clone());
            }
        }
        models
    }
}
