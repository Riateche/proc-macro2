#![allow(unused_attributes)]

extern crate proc_macro;

use std::mem;

#[cfg_attr(not(target_pointer_width = "64"), ignore = "only applicable to 64-bit")]
#[cfg_attr(randomize_layout, ignore = "disabled due to randomized layout")]
#[test]
fn test_proc_macro_size() {
    assert_eq!(mem::size_of::<proc_macro::Span>(), 4);
    assert_eq!(mem::size_of::<Option<proc_macro::Span>>(), 4);
    assert_eq!(mem::size_of::<proc_macro::Group>(), 20);
    assert_eq!(mem::size_of::<proc_macro::Ident>(), 12);
    assert_eq!(mem::size_of::<proc_macro::Punct>(), 8);
    assert_eq!(mem::size_of::<proc_macro::Literal>(), 16);
    assert_eq!(mem::size_of::<proc_macro::TokenStream>(), 4);
}

#[cfg_attr(not(target_pointer_width = "64"), ignore = "only applicable to 64-bit")]
#[cfg_attr(randomize_layout, ignore = "disabled due to randomized layout")]
#[cfg_attr(span_locations, ignore = "span locations are on")]
#[test]
fn test_proc_macro2_fallback_size_without_locations() {
    assert_eq!(mem::size_of::<proc_macro2_send::Span>(), 0);
    assert_eq!(mem::size_of::<Option<proc_macro2_send::Span>>(), 1);
    assert_eq!(mem::size_of::<proc_macro2_send::Group>(), 16);
    assert_eq!(mem::size_of::<proc_macro2_send::Ident>(), 24);
    assert_eq!(mem::size_of::<proc_macro2_send::Punct>(), 8);
    assert_eq!(mem::size_of::<proc_macro2_send::Literal>(), 24);
    assert_eq!(mem::size_of::<proc_macro2_send::TokenStream>(), 8);
}

#[cfg_attr(not(target_pointer_width = "64"), ignore = "only applicable to 64-bit")]
#[cfg_attr(randomize_layout, ignore = "disabled due to randomized layout")]
#[cfg_attr(not(span_locations), ignore = "span locations are off")]
#[test]
fn test_proc_macro2_fallback_size_with_locations() {
    assert_eq!(mem::size_of::<proc_macro2_send::Span>(), 8);
    assert_eq!(mem::size_of::<Option<proc_macro2_send::Span>>(), 12);
    assert_eq!(mem::size_of::<proc_macro2_send::Group>(), 24);
    assert_eq!(mem::size_of::<proc_macro2_send::Ident>(), 32);
    assert_eq!(mem::size_of::<proc_macro2_send::Punct>(), 16);
    assert_eq!(mem::size_of::<proc_macro2_send::Literal>(), 32);
    assert_eq!(mem::size_of::<proc_macro2_send::TokenStream>(), 8);
}

#[rustversion::attr(before(1.71), ignore = "requires Rust 1.71+")]
#[cfg_attr(not(target_pointer_width = "64"), ignore = "only applicable to 64-bit")]
#[cfg_attr(randomize_layout, ignore = "disabled due to randomized layout")]
#[cfg_attr(span_locations, ignore = "span locations are on")]
#[test]
fn test_proc_macro2_wrapper_size_without_locations() {
    assert_eq!(mem::size_of::<proc_macro2_send::Span>(), 4);
    assert_eq!(mem::size_of::<Option<proc_macro2_send::Span>>(), 8);
    assert_eq!(mem::size_of::<proc_macro2_send::Group>(), 24);
    assert_eq!(mem::size_of::<proc_macro2_send::Ident>(), 24);
    assert_eq!(mem::size_of::<proc_macro2_send::Punct>(), 12);
    assert_eq!(mem::size_of::<proc_macro2_send::Literal>(), 24);
    assert_eq!(mem::size_of::<proc_macro2_send::TokenStream>(), 32);
}

#[cfg_attr(not(target_pointer_width = "64"), ignore = "only applicable to 64-bit")]
#[cfg_attr(randomize_layout, ignore = "disabled due to randomized layout")]
#[cfg_attr(not(span_locations), ignore = "span locations are off")]
#[test]
fn test_proc_macro2_wrapper_size_with_locations() {
    assert_eq!(mem::size_of::<proc_macro2_send::Span>(), 12);
    assert_eq!(mem::size_of::<Option<proc_macro2_send::Span>>(), 12);
    assert_eq!(mem::size_of::<proc_macro2_send::Group>(), 32);
    assert_eq!(mem::size_of::<proc_macro2_send::Ident>(), 32);
    assert_eq!(mem::size_of::<proc_macro2_send::Punct>(), 20);
    assert_eq!(mem::size_of::<proc_macro2_send::Literal>(), 32);
    assert_eq!(mem::size_of::<proc_macro2_send::TokenStream>(), 32);
}
