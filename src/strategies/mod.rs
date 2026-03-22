pub mod html;
pub mod pdf;
pub mod selector;
pub mod strategy;

pub use html::HtmlStrategy;
pub use pdf::PdfStrategy;
pub use selector::select_strategy;
pub use strategy::OutputStrategy;
