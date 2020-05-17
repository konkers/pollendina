extern crate proc_macro;

#[proc_macro_derive(Matcher)]
pub fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let _ = input;
    unimplemented!()
}
