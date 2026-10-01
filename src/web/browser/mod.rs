//! Web 工作台内置浏览器面板的服务端：WebSocket 画面推送与输入转发。

mod commands;
mod frame_delivery;
mod input_queue;
mod protocol;
mod socket;

#[cfg(test)]
mod tests;

pub(crate) use socket::serve_socket;

#[cfg(test)]
mod input_queue_tests;

#[cfg(test)]
mod frame_delivery_tests;

#[cfg(test)]
mod socket_tests;
