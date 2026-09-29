use super::*;

/// 【模型接入】【默认选择】读取运行时同样使用的默认接入。
/// 参数: config 为配置，kind 为类型；返回: 接入标识或空
pub(super) fn active_id(config: &AppConfig, kind: ModelEndpointKind) -> Option<&str> {
    if kind == ModelEndpointKind::Jev {
        return config
            .jev_endpoint()
            .ok()
            .flatten()
            .map(|item| item.id.as_str());
    }
    config
        .model_endpoints
        .iter()
        .find(|item| item.kind == kind)
        .map(|item| item.id.as_str())
}

/// 【模型接入】【创建草稿】生成唯一标识和可编辑默认值，不写入配置。
/// 参数: config 为已有配置，kind 为类型；返回: 新接入草稿
pub(super) fn new_endpoint(config: &AppConfig, kind: ModelEndpointKind) -> ModelEndpointConfig {
    let prefix = if kind == ModelEndpointKind::Jev {
        "jev"
    } else {
        "image"
    };
    let id = (1..)
        .map(|index| format!("{prefix}-{index}"))
        .find(|id| config.model_endpoints.iter().all(|item| &item.id != id))
        .unwrap();
    ModelEndpointConfig {
        id,
        kind,
        name: if kind == ModelEndpointKind::Jev {
            "TypeSafe Jev"
        } else {
            "Image generation"
        }
        .into(),
        endpoint: if kind == ModelEndpointKind::Jev {
            crate::config::JEV_OFFICIAL_ENDPOINT
        } else {
            "https://api.openai.com/v1/images/generations"
        }
        .into(),
        protocol: "auto".into(),
        model: if kind == ModelEndpointKind::Jev {
            crate::config::JEV_DEFAULT_MODEL
        } else {
            "gpt-image-1"
        }
        .into(),
        api_key: String::new(),
        api_keys: Vec::new(),
        api_key_selected: None,
        api_key_balance: false,
        models: Vec::new(),
    }
}

/// 【模型接入】【保存草稿】整体校验后替换接入，不覆盖其他接入的配置。
/// 参数: config 为会话配置，item 为草稿；返回: 校验结果
pub(super) fn save(config: &mut AppConfig, item: ModelEndpointConfig) -> Result<()> {
    let mut next = config.clone();
    if let Some(index) = next
        .model_endpoints
        .iter()
        .position(|old| old.id == item.id)
    {
        next.model_endpoints[index] = item;
    } else {
        next.model_endpoints.push(item);
    }
    next.validate()?;
    *config = next;
    Ok(())
}

/// 【模型接入】【启用接入】设置 Jev 引用或生图默认排序，与运行时选择规则一致。
/// 参数: config 为草稿，id 为已存在接入标识；返回: 无
pub(super) fn activate(config: &mut AppConfig, id: &str) {
    let Some(index) = config.model_endpoints.iter().position(|item| item.id == id) else {
        return;
    };
    if config.model_endpoints[index].kind == ModelEndpointKind::Jev {
        config.jev.endpoint_id = id.into();
    } else {
        let item = config.model_endpoints.remove(index);
        config.model_endpoints.insert(0, item);
    }
}

/// 【模型接入】【删除接入】清理对应 Jev 引用，其他类型的默认选择保持稳定。
/// 参数: config 为草稿，id 为接入标识；返回: 无
pub(super) fn remove(config: &mut AppConfig, id: &str) {
    config.model_endpoints.retain(|item| item.id != id);
    if config.jev.endpoint_id == id {
        config.jev.endpoint_id.clear();
    }
}
