extern crate proc_macro;

use proc_macro_hack::proc_macro_hack;

#[proc_macro_hack]
pub use match_macro_impl::match_widget;

mod matcher;
pub use matcher::WidgetMatcher;
