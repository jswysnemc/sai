use super::defaults::{
    default_qq_gateway_base_url, default_qq_gateway_listen, default_qq_gateway_transport,
    default_weixin_gateway_base_url, default_weixin_gateway_bot_type,
    default_weixin_gateway_cdn_base_url,
};
use super::model::{GatewayConfig, QqGatewayConfig, WeixinGatewayConfig};

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            qq: QqGatewayConfig::default(),
            weixin: WeixinGatewayConfig::default(),
        }
    }
}

impl Default for QqGatewayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            transport: default_qq_gateway_transport(),
            listen: default_qq_gateway_listen(),
            base_url: default_qq_gateway_base_url(),
            token: String::new(),
            app_id: String::new(),
            client_secret: String::new(),
        }
    }
}

impl Default for WeixinGatewayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: default_weixin_gateway_base_url(),
            cdn_base_url: default_weixin_gateway_cdn_base_url(),
            bot_type: default_weixin_gateway_bot_type(),
            token: String::new(),
            account: String::new(),
            bot_agent: String::new(),
        }
    }
}
