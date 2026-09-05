use super::*;

impl ProviderBrowser<'_> {
    /// 刷新当前供应商的本地与远程模型，无参数，返回值为空。
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
        self.orgs = vec!["All".to_string()];
        self.models.clear();
        self.rebuild_models();
        self.fetch_seq += 1;
        if let Some(provider) = self.config.providers.get(self.provider_idx).cloned() {
            let seq = self.fetch_seq;
            let (tx, rx) = mpsc::channel();
            self.fetch_rx = Some(rx);
            self.loading = true;
            self.status = t("Fetching model list...", "正在获取模型列表...").to_string();
            std::thread::spawn(move || {
                let result = fetch_models(&provider).map_err(|err| err.to_string());
                let _ = tx.send((seq, result));
            });
        } else {
            self.fetch_rx = None;
            self.loading = false;
            self.status.clear();
        }
        self.org_idx = 0;
        self.model_idx = 0;
    }

    /// 合并本次远程获取结果并重建候选，无参数，返回值为空。
    pub(super) fn poll_fetch_result(&mut self) {
        let Some(rx) = &self.fetch_rx else {
            return;
        };
        let Ok((seq, result)) = rx.try_recv() else {
            return;
        };
        if seq != self.fetch_seq {
            return;
        }
        self.loading = false;
        self.fetch_rx = None;
        match result {
            Ok(result) => {
                self.status = format!(
                    "{} {} {}",
                    t("Fetched", "已获取"),
                    result.models.len(),
                    t("models", "个模型")
                );
                for model in result.models {
                    if !self.raw_models.iter().any(|item| item == &model) {
                        self.raw_models.push(model);
                    }
                }
                self.remote_metadata = result.metadata;
            }
            Err(err) => {
                self.status = format_status_line(&format!(
                    "{}: {err}",
                    t("Failed to fetch models", "获取模型失败")
                ));
            }
        }
        self.rebuild_models();
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
        self.org_idx = self.org_idx.min(self.orgs.len().saturating_sub(1));
        self.models = grouped.remove(&self.orgs[self.org_idx]).unwrap_or_default();
        self.model_idx = self.model_idx.min(self.models.len().saturating_sub(1));
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
