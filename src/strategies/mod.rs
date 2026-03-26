pub mod docx;
pub mod html;
pub mod pdf;
pub mod selector;
pub mod strategy;

pub use docx::DocxStrategy;
pub use html::HtmlStrategy;
pub use pdf::PdfStrategy;
pub use selector::select_strategy;
pub use strategy::{OutputStrategy, RenderContext};
