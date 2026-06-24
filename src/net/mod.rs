//! picals-crawler 网络基础设施

mod catalog;
mod client;
mod event;
mod policy;
mod session;
mod state;
pub(crate) mod test_hook;
pub(crate) mod transfer;

pub use catalog::{CurrentUserPage, HostKind, RequestKind};
pub use event::{NetEvent, SessionObserver};
pub use policy::{is_cooldown_status, is_retryable_http_status};
pub use session::{PixivNetSession, PixivNetSessionBuilder, resolve_base_url};
