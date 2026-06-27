use proc_macro2_send::Span;

fn main() {
    fn requires_send<T: Send>() {}
    requires_send::<Span>();
}
