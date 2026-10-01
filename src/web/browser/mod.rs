//! Web 工作台内置浏览器面板的服务端：WebSocket 画面推送与输入转发。

mod protocol;
mod socket;

#[cfg(test)]
mod tests;

pub(crate) use socket::serve_socket;
