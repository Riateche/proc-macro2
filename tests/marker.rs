#![allow(clippy::extra_unused_type_parameters)]

use proc_macro2_send::{
    Delimiter, Group, Ident, LexError, Literal, Punct, Spacing, Span, TokenStream, TokenTree,
};

macro_rules! assert_impl {
    ($ty:ident is $($marker:ident) and +) => {
        #[test]
        #[allow(non_snake_case)]
        fn $ty() {
            fn assert_implemented<T: $($marker +)+>() {}
            assert_implemented::<$ty>();
        }
    };

    ($ty:ident is not $($marker:ident) or +) => {
        #[test]
        #[allow(non_snake_case)]
        fn $ty() {
            $(
                {
                    // Implemented for types that implement $marker.
                    #[allow(dead_code)]
                    trait IsNotImplemented {
                        fn assert_not_implemented() {}
                    }
                    impl<T: $marker> IsNotImplemented for T {}

                    // Implemented for the type being tested.
                    trait IsImplemented {
                        fn assert_not_implemented() {}
                    }
                    impl IsImplemented for $ty {}

                    // If $ty does not implement $marker, there is no ambiguity
                    // in the following trait method call.
                    <$ty>::assert_not_implemented();
                }
            )+
        }
    };
}

assert_impl!(Delimiter is Send and Sync);
assert_impl!(Spacing is Send and Sync);

assert_impl!(Group is Send and Sync);
assert_impl!(Ident is Send and Sync);
assert_impl!(LexError is Send and Sync);
assert_impl!(Literal is Send and Sync);
assert_impl!(Punct is Send and Sync);
assert_impl!(Span is Send and Sync);
assert_impl!(TokenStream is Send and Sync);
assert_impl!(TokenTree is Send and Sync);

#[cfg(procmacro2_semver_exempt)]
mod semver_exempt {
    use proc_macro2_send::LineColumn;

    assert_impl!(LineColumn is Send and Sync);
}

mod unwind_safe {
    #[cfg(procmacro2_semver_exempt)]
    use proc_macro2_send::LineColumn;
    use proc_macro2_send::{
        Delimiter, Group, Ident, LexError, Literal, Punct, Spacing, Span, TokenStream, TokenTree,
    };
    use std::panic::{RefUnwindSafe, UnwindSafe};

    macro_rules! assert_unwind_safe {
        ($($types:ident)*) => {
            $(
                assert_impl!($types is UnwindSafe and RefUnwindSafe);
            )*
        };
    }

    assert_unwind_safe! {
        Delimiter
        Group
        Ident
        LexError
        Literal
        Punct
        Spacing
        Span
        TokenStream
        TokenTree
    }

    #[cfg(procmacro2_semver_exempt)]
    assert_unwind_safe! {
        LineColumn
    }
}
