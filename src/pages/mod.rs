//! Routed pages. Each is a top-level `#[component]` mounted by `app::Route`.

mod about;
mod article;
mod complaints_policy;
mod contact;
mod corrections_log;
mod database;
mod desk;
mod governance;
mod home;
mod news;
mod notfound;
mod podcast;
mod privacy;
mod standards;
mod team;
mod watch;

pub use about::About;
pub use article::Article;
pub use complaints_policy::ComplaintsPolicy;
pub use contact::Contact;
pub use corrections_log::CorrectionsLog;
pub use database::Database;
pub use desk::{Desk, DeskForgot, DeskPreview, DeskReset, WriteArticle};
pub use governance::Governance;
pub use home::Home;
pub use news::News;
pub use notfound::NotFound;
pub use podcast::Podcast;
pub use privacy::Privacy;
pub use standards::{ComplaintPage, Standards};
pub use team::Team;
pub use watch::Watch;
