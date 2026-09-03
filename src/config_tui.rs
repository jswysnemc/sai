mod agents;
mod form;
mod gateways;
mod input;
mod knowledge;
mod layout;
mod model_metadata_form;
mod multi_select;
mod plugin_fields;
mod plugins;
mod provider_fetch;
mod provider_forms;
mod providers;
mod session;
mod settings;
mod skills;
mod theme;
mod ui;
mod web_search_fields;

pub use session::run;

// 供 models picker 等模块复用同一份供应商/模型候选与解析逻辑
pub(crate) use form::{parse_provider_model_choice, provider_model_choice_values};
