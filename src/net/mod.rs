//! picals-crawler 网络基础设施

mod catalog;
mod client;
mod event;
mod policy;
mod session;
mod state;
pub(crate) mod test_hook;
pub(crate) mod transfer;

pub use catalog::RequestKind;
pub use event::{NetEvent, SessionObserver};
pub use session::PixivNetSession;

pub(crate) use catalog::HostKind;
pub(crate) use policy::{is_cooldown_status, is_retryable_http_status};
pub use session::PixivNetSessionBuilder;
pub(crate) use session::resolve_base_url;
