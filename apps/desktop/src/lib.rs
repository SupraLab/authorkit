//! AuthorKit desktop library — exposed so integration tests can drive the app
//! without launching a native window.

pub mod app;
pub mod brand;
pub mod chat_md;
pub mod chat_store;
pub mod i18n;
pub mod md_layout;
pub mod ms_tools;
pub mod panels;
pub mod providers;
pub mod theme;
pub mod widgets;

pub use app::AuthorKitApp;
pub use theme::UiPrefs;
