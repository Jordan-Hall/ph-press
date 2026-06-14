//! Routed pages. Each is a top-level `#[component]` mounted by `app::Route`.

mod about;
mod cases;
mod contact;
mod database;
mod home;
mod news;
mod notfound;
mod podcast;
mod privacy;
mod standards;
mod watch;

pub use about::About;
pub use cases::Cases;
pub use contact::Contact;
pub use database::Database;
pub use home::Home;
pub use news::News;
pub use notfound::NotFound;
pub use podcast::Podcast;
pub use privacy::Privacy;
pub use standards::Standards;
pub use watch::Watch;
