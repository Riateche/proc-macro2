#[cfg(span_locations)]
use crate::location::LineColumn;
use crate::parse::{self, Cursor};
use crate::rcvec::{RcVec, RcVecBuilder, RcVecIntoIter, RcVecMut};
#[cfg(span_locations)]
use crate::source::Source;
use crate::{Delimiter, Spacing, TokenTree};
use alloc::borrow::ToOwned as _;
use alloc::format;
use alloc::string::{String, ToString as _};
use alloc::vec::Vec;
#[cfg(span_locations)]
use core::cmp;
use core::cmp::Ordering;
use core::ffi::CStr;
use core::fmt::{self, Debug, Display, Write};
use core::mem::ManuallyDrop;
#[cfg(span_locations)]
use core::ops::Range;
use core::ops::RangeBounds;
use core::ptr;
use core::str;
#[cfg(feature = "proc-macro")]
use core::str::FromStr;
#[cfg(span_locations)]
use std::path::PathBuf;
use std::sync::Arc;

/// Force use of proc-macro2's fallback implementation of the API for now, even
/// if the compiler's implementation is available.
pub fn force() {}

/// Resume using the compiler's implementation of the proc macro API if it is
/// available.
pub fn unforce() {}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct TokenStream {
    inner: RcVec<TokenTree>,
}

#[derive(Debug)]
pub(crate) struct LexError {
    pub(crate) span: crate::Span,
}

impl LexError {
    pub(crate) fn span(&self) -> &crate::Span {
        &self.span
    }

    pub(crate) fn call_site() -> Self {
        LexError {
            span: crate::Span::_new(Span::call_site()),
        }
    }
}

impl TokenStream {
    pub(crate) fn new() -> Self {
        TokenStream {
            inner: RcVecBuilder::new().build(),
        }
    }

    pub fn parse_with_name(name: &str, source_text: &str) -> Result<Self, LexError> {
        #[cfg(span_locations)]
        let source = Source::new(name, source_text);
        #[cfg(span_locations)]
        let mut cursor = get_cursor(&source);
        #[cfg(not(span_locations))]
        let _ = name;
        #[cfg(not(span_locations))]
        let mut cursor = get_cursor(source_text);

        // Strip a byte order mark if present
        const BYTE_ORDER_MARK: &str = "\u{feff}";
        if cursor.starts_with(BYTE_ORDER_MARK) {
            cursor = cursor.advance(BYTE_ORDER_MARK.len());
        }

        parse::token_stream(cursor)
    }

    #[cfg(feature = "proc-macro")]
    pub(crate) fn from_str_unchecked(src: &str) -> Self {
        Self::parse_with_name("", src).unwrap()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.inner.len() == 0
    }

    fn take_inner(self) -> RcVecBuilder<TokenTree> {
        let nodrop = ManuallyDrop::new(self);
        unsafe { ptr::read(&nodrop.inner) }.make_owned()
    }
}

fn push_token_from_proc_macro(mut vec: RcVecMut<TokenTree>, token: TokenTree) {
    // https://github.com/dtolnay/proc-macro2/issues/235
    match token {
        TokenTree::Literal(crate::Literal { inner: literal, .. })
            if literal.repr.starts_with('-') =>
        {
            push_negative_literal(vec, literal);
        }
        _ => vec.push(token),
    }

    #[cold]
    fn push_negative_literal(mut vec: RcVecMut<TokenTree>, mut literal: Literal) {
        literal.repr.remove(0);
        let mut punct = crate::Punct::new('-', Spacing::Alone);
        punct.set_span(literal.span.clone());
        vec.push(TokenTree::Punct(punct));
        vec.push(TokenTree::Literal(crate::Literal::_new_fallback(literal)));
    }
}

// Nonrecursive to prevent stack overflow.
impl Drop for TokenStream {
    fn drop(&mut self) {
        let mut stack = Vec::new();
        let mut current = match self.inner.get_mut() {
            Some(inner) => inner.take().into_iter(),
            None => return,
        };
        loop {
            while let Some(token) = current.next() {
                let group = match token {
                    TokenTree::Group(group) => group.inner,
                    _ => continue,
                };
                let mut group = group;
                if let Some(inner) = group.stream.inner.get_mut() {
                    stack.push(current);
                    current = inner.take().into_iter();
                }
            }
            match stack.pop() {
                Some(next) => current = next,
                None => return,
            }
        }
    }
}

pub(crate) struct TokenStreamBuilder {
    inner: RcVecBuilder<TokenTree>,
}

impl TokenStreamBuilder {
    pub(crate) fn new() -> Self {
        TokenStreamBuilder {
            inner: RcVecBuilder::new(),
        }
    }

    pub(crate) fn with_capacity(cap: usize) -> Self {
        TokenStreamBuilder {
            inner: RcVecBuilder::with_capacity(cap),
        }
    }

    pub(crate) fn push_token_from_parser(&mut self, tt: TokenTree) {
        self.inner.push(tt);
    }

    pub(crate) fn build(self) -> TokenStream {
        TokenStream {
            inner: self.inner.build(),
        }
    }
}

#[cfg(span_locations)]
fn get_cursor(source: &Source) -> Cursor {
    #[cfg(fuzzing)]
    return Cursor { rest: src, off: 1 };

    // Create a dummy file & add it to the source map
    #[cfg(not(fuzzing))]
    Cursor {
        rest: source.source_text(),
        off: 0,
        source,
    }
}

#[cfg(not(span_locations))]
fn get_cursor(src: &str) -> Cursor {
    Cursor { rest: src }
}

impl Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("cannot parse string into token stream")
    }
}

impl Display for TokenStream {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut joint = false;
        for (i, tt) in self.inner.iter().enumerate() {
            if i != 0 && !joint {
                write!(f, " ")?;
            }
            joint = false;
            match tt {
                TokenTree::Group(tt) => write!(f, "{}", tt),
                TokenTree::Ident(tt) => write!(f, "{}", tt),
                TokenTree::Punct(tt) => {
                    joint = tt.spacing() == Spacing::Joint;
                    write!(f, "{}", tt)
                }
                TokenTree::Literal(tt) => write!(f, "{}", tt),
            }?;
        }

        Ok(())
    }
}

impl Debug for TokenStream {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("TokenStream ")?;
        f.debug_list().entries(self.clone()).finish()
    }
}

#[cfg(feature = "proc-macro")]
impl From<proc_macro::TokenStream> for TokenStream {
    fn from(inner: proc_macro::TokenStream) -> Self {
        TokenStream::from_str_unchecked(&inner.to_string())
    }
}

#[cfg(feature = "proc-macro")]
impl From<TokenStream> for proc_macro::TokenStream {
    fn from(inner: TokenStream) -> Self {
        proc_macro::TokenStream::from_str_unchecked(&inner.to_string())
    }
}

impl From<TokenTree> for TokenStream {
    fn from(tree: TokenTree) -> Self {
        let mut stream = RcVecBuilder::new();
        push_token_from_proc_macro(stream.as_mut(), tree);
        TokenStream {
            inner: stream.build(),
        }
    }
}

impl FromIterator<TokenTree> for TokenStream {
    fn from_iter<I: IntoIterator<Item = TokenTree>>(tokens: I) -> Self {
        let mut stream = TokenStream::new();
        stream.extend(tokens);
        stream
    }
}

impl FromIterator<TokenStream> for TokenStream {
    fn from_iter<I: IntoIterator<Item = TokenStream>>(streams: I) -> Self {
        let mut v = RcVecBuilder::new();

        for stream in streams {
            v.extend(stream.take_inner());
        }

        TokenStream { inner: v.build() }
    }
}

impl Extend<TokenTree> for TokenStream {
    fn extend<I: IntoIterator<Item = TokenTree>>(&mut self, tokens: I) {
        let mut vec = self.inner.make_mut();
        tokens
            .into_iter()
            .for_each(|token| push_token_from_proc_macro(vec.as_mut(), token));
    }
}

impl Extend<TokenStream> for TokenStream {
    fn extend<I: IntoIterator<Item = TokenStream>>(&mut self, streams: I) {
        self.inner.make_mut().extend(streams.into_iter().flatten());
    }
}

pub(crate) type TokenTreeIter = RcVecIntoIter<TokenTree>;

impl IntoIterator for TokenStream {
    type Item = TokenTree;
    type IntoIter = TokenTreeIter;

    fn into_iter(self) -> TokenTreeIter {
        self.take_inner().into_iter()
    }
}

#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Span {
    #[cfg(span_locations)]
    pub(crate) lo: u32,
    #[cfg(span_locations)]
    pub(crate) hi: u32,
    #[cfg(span_locations)]
    pub(crate) source: Source,
}

impl Span {
    #[cfg(not(span_locations))]
    pub(crate) fn call_site() -> Self {
        Span {}
    }

    #[cfg(span_locations)]
    pub(crate) fn call_site() -> Self {
        Span {
            lo: 0,
            hi: 0,
            source: crate::source::EMPTY_SOURCE.clone(),
        }
    }

    pub(crate) fn mixed_site() -> Self {
        Span::call_site()
    }

    #[cfg(procmacro2_semver_exempt)]
    pub(crate) fn def_site() -> Self {
        Span::call_site()
    }

    pub(crate) fn resolved_at(&self, _other: &Span) -> Span {
        // Stable spans consist only of line/column information, so
        // `resolved_at` and `located_at` only select which span the
        // caller wants line/column information from.
        self.clone()
    }

    pub(crate) fn located_at(&self, other: &Span) -> Span {
        other.clone()
    }

    #[cfg(span_locations)]
    pub(crate) fn byte_range(&self) -> Range<usize> {
        #[cfg(fuzzing)]
        return 0..0;

        #[cfg(not(fuzzing))]
        self.source.byte_range(self)
    }

    #[cfg(span_locations)]
    pub(crate) fn start(&self) -> LineColumn {
        #[cfg(fuzzing)]
        return LineColumn { line: 0, column: 0 };

        #[cfg(not(fuzzing))]
        self.source.offset_line_column(self.lo as usize)
    }

    #[cfg(span_locations)]
    pub(crate) fn end(&self) -> LineColumn {
        #[cfg(fuzzing)]
        return LineColumn { line: 0, column: 0 };

        #[cfg(not(fuzzing))]
        self.source.offset_line_column(self.hi as usize)
    }

    #[cfg(span_locations)]
    pub(crate) fn file(&self) -> &Arc<str> {
        #[cfg(fuzzing)]
        return "<unspecified>".to_owned();

        #[cfg(not(fuzzing))]
        self.source.name()
    }

    #[cfg(span_locations)]
    pub(crate) fn local_file(&self) -> Option<PathBuf> {
        None
    }

    #[cfg(not(span_locations))]
    pub(crate) fn join(&self, _other: &Span) -> Option<Span> {
        Some(Span {})
    }

    #[cfg(span_locations)]
    pub(crate) fn join(&self, other: &Span) -> Option<Span> {
        #[cfg(fuzzing)]
        return {
            let _ = other;
            None
        };

        #[cfg(not(fuzzing))]
        {
            if self.source != other.source {
                return None;
            }
            Some(Span {
                lo: cmp::min(self.lo, other.lo),
                hi: cmp::max(self.hi, other.hi),
                source: self.source.clone(),
            })
        }
    }

    #[cfg(span_locations)]
    pub(crate) fn source_text(&self) -> &str {
        let byte_range = self.byte_range();
        &self.source.source_text()[byte_range]
    }

    #[cfg(span_locations)]
    pub(crate) fn file_content(&self) -> &Arc<str> {
        self.source.source_text()
    }

    #[cfg(not(span_locations))]
    pub(crate) fn first_byte(&self) -> Self {
        self.clone()
    }

    #[cfg(span_locations)]
    pub(crate) fn first_byte(&self) -> Self {
        Span {
            lo: self.lo,
            hi: cmp::min(self.lo.saturating_add(1), self.hi),
            source: self.source.clone(),
        }
    }

    #[cfg(not(span_locations))]
    pub(crate) fn last_byte(&self) -> Self {
        self.clone()
    }

    #[cfg(span_locations)]
    pub(crate) fn last_byte(&self) -> Self {
        Span {
            lo: cmp::max(self.hi.saturating_sub(1), self.lo),
            hi: self.hi,
            source: self.source.clone(),
        }
    }

    #[cfg(span_locations)]
    fn is_call_site(&self) -> bool {
        self.lo == 0 && self.hi == 0
    }
}

impl Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[cfg(span_locations)]
        return write!(f, "bytes({}..{})", self.lo, self.hi);

        #[cfg(not(span_locations))]
        write!(f, "Span")
    }
}

pub(crate) fn debug_span_field_if_nontrivial(debug: &mut fmt::DebugStruct, span: &Span) {
    #[cfg(span_locations)]
    {
        if span.is_call_site() {
            return;
        }
    }

    if cfg!(span_locations) {
        debug.field("span", &span);
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Group {
    delimiter: Delimiter,
    stream: TokenStream,
    span: crate::Span,
}

impl Group {
    pub(crate) fn new(delimiter: Delimiter, stream: TokenStream) -> Self {
        Group {
            delimiter,
            stream,
            span: crate::Span::_new(Span::call_site()),
        }
    }

    pub(crate) fn delimiter(&self) -> Delimiter {
        self.delimiter
    }

    pub(crate) fn stream(&self) -> TokenStream {
        self.stream.clone()
    }

    pub(crate) fn span(&self) -> &crate::Span {
        &self.span
    }

    pub(crate) fn span_open(&self) -> Span {
        self.span.inner.first_byte()
    }

    pub(crate) fn span_close(&self) -> Span {
        self.span.inner.last_byte()
    }

    pub(crate) fn set_span(&mut self, span: Span) {
        self.span = crate::Span::_new(span);
    }
}

impl Display for Group {
    // We attempt to match libproc_macro's formatting.
    // Empty parens: ()
    // Nonempty parens: (...)
    // Empty brackets: []
    // Nonempty brackets: [...]
    // Empty braces: { }
    // Nonempty braces: { ... }
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (open, close) = match self.delimiter {
            Delimiter::Parenthesis => ("(", ")"),
            Delimiter::Brace => ("{ ", "}"),
            Delimiter::Bracket => ("[", "]"),
            Delimiter::None => ("", ""),
        };

        f.write_str(open)?;
        Display::fmt(&self.stream, f)?;
        if self.delimiter == Delimiter::Brace && !self.stream.inner.is_empty() {
            f.write_str(" ")?;
        }
        f.write_str(close)?;

        Ok(())
    }
}

impl Debug for Group {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut debug = fmt.debug_struct("Group");
        debug.field("delimiter", &self.delimiter);
        debug.field("stream", &self.stream);
        debug_span_field_if_nontrivial(&mut debug, &self.span.inner);
        debug.finish()
    }
}

#[derive(Clone, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Ident {
    #[cfg_attr(feature = "serde", serde(with = "serde_rc"))]
    sym: Arc<str>,
    span: crate::Span,
    raw: bool,
}

impl Ident {
    #[track_caller]
    pub(crate) fn new_checked(string: &str, span: Span) -> Self {
        validate_ident(string);
        Ident::new_unchecked(string, span)
    }

    pub(crate) fn new_unchecked(string: &str, span: Span) -> Self {
        Ident {
            sym: string.into(),
            span: crate::Span::_new(span),
            raw: false,
        }
    }

    #[track_caller]
    pub(crate) fn new_raw_checked(string: &str, span: Span) -> Self {
        validate_ident_raw(string);
        Ident::new_raw_unchecked(string, span)
    }

    pub(crate) fn new_raw_unchecked(string: &str, span: Span) -> Self {
        Ident {
            sym: string.into(),
            span: crate::Span::_new(span),
            raw: true,
        }
    }

    pub(crate) fn span(&self) -> &crate::Span {
        &self.span
    }

    pub(crate) fn set_span(&mut self, span: Span) {
        self.span = crate::Span::_new(span);
    }
}

impl PartialOrd for Ident {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Ident {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.raw, &self.sym).cmp(&(other.raw, &other.sym))
    }
}

pub(crate) fn is_ident_start(c: char) -> bool {
    c == '_' || unicode_ident::is_xid_start(c)
}

pub(crate) fn is_ident_continue(c: char) -> bool {
    unicode_ident::is_xid_continue(c)
}

#[track_caller]
fn validate_ident(string: &str) {
    if string.is_empty() {
        panic!("Ident is not allowed to be empty; use Option<Ident>");
    }

    if string.bytes().all(|digit| b'0' <= digit && digit <= b'9') {
        panic!("Ident cannot be a number; use Literal instead");
    }

    fn ident_ok(string: &str) -> bool {
        let mut chars = string.chars();
        let first = chars.next().unwrap();
        if !is_ident_start(first) {
            return false;
        }
        for ch in chars {
            if !is_ident_continue(ch) {
                return false;
            }
        }
        true
    }

    if !ident_ok(string) {
        panic!("{:?} is not a valid Ident", string);
    }
}

#[track_caller]
fn validate_ident_raw(string: &str) {
    validate_ident(string);

    match string {
        "_" | "super" | "self" | "Self" | "crate" => {
            panic!("`r#{}` cannot be a raw identifier", string);
        }
        _ => {}
    }
}

impl PartialEq for Ident {
    fn eq(&self, other: &Ident) -> bool {
        self.sym == other.sym && self.raw == other.raw
    }
}

impl<T> PartialEq<T> for Ident
where
    T: ?Sized + AsRef<str>,
{
    fn eq(&self, other: &T) -> bool {
        let other = other.as_ref();
        if self.raw {
            other.starts_with("r#") && *self.sym == other[2..]
        } else {
            *self.sym == *other
        }
    }
}

impl Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.raw {
            f.write_str("r#")?;
        }
        f.write_str(&self.sym)
    }
}

#[allow(clippy::missing_fields_in_debug)]
impl Debug for Ident {
    // Ident(proc_macro), Ident(r#union)
    #[cfg(not(span_locations))]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut debug = f.debug_tuple("Ident");
        debug.field(&format_args!("{}", self));
        debug.finish()
    }

    // Ident {
    //     sym: proc_macro,
    //     span: bytes(128..138)
    // }
    #[cfg(span_locations)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut debug = f.debug_struct("Ident");
        debug.field("sym", &format_args!("{}", self));
        debug_span_field_if_nontrivial(&mut debug, &self.span.inner);
        debug.finish()
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Literal {
    pub(crate) repr: String,
    span: crate::Span,
}

macro_rules! suffixed_numbers {
    ($($name:ident => $kind:ident,)*) => ($(
        pub(crate) fn $name(n: $kind) -> Literal {
            Literal::_new(format!(concat!("{}", stringify!($kind)), n))
        }
    )*)
}

macro_rules! unsuffixed_numbers {
    ($($name:ident => $kind:ident,)*) => ($(
        pub(crate) fn $name(n: $kind) -> Literal {
            Literal::_new(n.to_string())
        }
    )*)
}

impl Literal {
    pub(crate) fn _new(repr: String) -> Self {
        Literal {
            repr,
            span: crate::Span::_new(Span::call_site()),
        }
    }

    pub(crate) fn from_str_checked(repr: &str) -> Result<Self, LexError> {
        #[cfg(span_locations)]
        let source = Source::new("", repr);
        #[cfg(span_locations)]
        let mut cursor = get_cursor(&source);
        #[cfg(span_locations)]
        let lo = cursor.off;

        #[cfg(not(span_locations))]
        let mut cursor = get_cursor(repr);

        let negative = cursor.starts_with_char('-');
        if negative {
            cursor = cursor.advance(1);
            if !cursor.starts_with_fn(|ch| ch.is_ascii_digit()) {
                return Err(LexError::call_site());
            }
        }

        if let Ok((rest, mut literal)) = parse::literal(cursor) {
            if rest.is_empty() {
                if negative {
                    literal.repr.insert(0, '-');
                }
                literal.span = crate::Span::_new(Span {
                    #[cfg(span_locations)]
                    lo,
                    #[cfg(span_locations)]
                    hi: rest.off,
                    #[cfg(span_locations)]
                    source,
                });
                return Ok(literal);
            }
        }
        Err(LexError::call_site())
    }

    pub(crate) unsafe fn from_str_unchecked(repr: &str) -> Self {
        Literal::_new(repr.to_owned())
    }

    suffixed_numbers! {
        u8_suffixed => u8,
        u16_suffixed => u16,
        u32_suffixed => u32,
        u64_suffixed => u64,
        u128_suffixed => u128,
        usize_suffixed => usize,
        i8_suffixed => i8,
        i16_suffixed => i16,
        i32_suffixed => i32,
        i64_suffixed => i64,
        i128_suffixed => i128,
        isize_suffixed => isize,

        f32_suffixed => f32,
        f64_suffixed => f64,
    }

    unsuffixed_numbers! {
        u8_unsuffixed => u8,
        u16_unsuffixed => u16,
        u32_unsuffixed => u32,
        u64_unsuffixed => u64,
        u128_unsuffixed => u128,
        usize_unsuffixed => usize,
        i8_unsuffixed => i8,
        i16_unsuffixed => i16,
        i32_unsuffixed => i32,
        i64_unsuffixed => i64,
        i128_unsuffixed => i128,
        isize_unsuffixed => isize,
    }

    pub(crate) fn f32_unsuffixed(f: f32) -> Literal {
        let mut s = f.to_string();
        if !s.contains('.') {
            s.push_str(".0");
        }
        Literal::_new(s)
    }

    pub(crate) fn f64_unsuffixed(f: f64) -> Literal {
        let mut s = f.to_string();
        if !s.contains('.') {
            s.push_str(".0");
        }
        Literal::_new(s)
    }

    pub(crate) fn string(string: &str) -> Literal {
        let mut repr = String::with_capacity(string.len() + 2);
        repr.push('"');
        escape_utf8(string, &mut repr);
        repr.push('"');
        Literal::_new(repr)
    }

    pub(crate) fn character(ch: char) -> Literal {
        let mut repr = String::new();
        repr.push('\'');
        if ch == '"' {
            // escape_debug turns this into '\"' which is unnecessary.
            repr.push(ch);
        } else {
            repr.extend(ch.escape_debug());
        }
        repr.push('\'');
        Literal::_new(repr)
    }

    pub(crate) fn byte_character(byte: u8) -> Literal {
        let mut repr = "b'".to_string();
        #[allow(clippy::match_overlapping_arm)]
        match byte {
            b'\0' => repr.push_str(r"\0"),
            b'\t' => repr.push_str(r"\t"),
            b'\n' => repr.push_str(r"\n"),
            b'\r' => repr.push_str(r"\r"),
            b'\'' => repr.push_str(r"\'"),
            b'\\' => repr.push_str(r"\\"),
            b'\x20'..=b'\x7E' => repr.push(byte as char),
            _ => {
                let _ = write!(repr, r"\x{:02X}", byte);
            }
        }
        repr.push('\'');
        Literal::_new(repr)
    }

    pub(crate) fn byte_string(bytes: &[u8]) -> Literal {
        let mut repr = "b\"".to_string();
        let mut bytes = bytes.iter();
        while let Some(&b) = bytes.next() {
            #[allow(clippy::match_overlapping_arm)]
            match b {
                b'\0' => repr.push_str(match bytes.as_slice().first() {
                    // circumvent clippy::octal_escapes lint
                    Some(b'0'..=b'7') => r"\x00",
                    _ => r"\0",
                }),
                b'\t' => repr.push_str(r"\t"),
                b'\n' => repr.push_str(r"\n"),
                b'\r' => repr.push_str(r"\r"),
                b'"' => repr.push_str("\\\""),
                b'\\' => repr.push_str(r"\\"),
                b'\x20'..=b'\x7E' => repr.push(b as char),
                _ => {
                    let _ = write!(repr, r"\x{:02X}", b);
                }
            }
        }
        repr.push('"');
        Literal::_new(repr)
    }

    pub(crate) fn c_string(string: &CStr) -> Literal {
        let mut repr = "c\"".to_string();
        let mut bytes = string.to_bytes();
        while !bytes.is_empty() {
            let (valid, invalid) = match str::from_utf8(bytes) {
                Ok(all_valid) => {
                    bytes = b"";
                    (all_valid, bytes)
                }
                Err(utf8_error) => {
                    let (valid, rest) = bytes.split_at(utf8_error.valid_up_to());
                    let valid = str::from_utf8(valid).unwrap();
                    let invalid = utf8_error
                        .error_len()
                        .map_or(rest, |error_len| &rest[..error_len]);
                    bytes = &bytes[valid.len() + invalid.len()..];
                    (valid, invalid)
                }
            };
            escape_utf8(valid, &mut repr);
            for &byte in invalid {
                let _ = write!(repr, r"\x{:02X}", byte);
            }
        }
        repr.push('"');
        Literal::_new(repr)
    }

    pub(crate) fn span(&self) -> &crate::Span {
        &self.span
    }

    pub(crate) fn set_span(&mut self, span: Span) {
        self.span = crate::Span::_new(span);
    }

    pub(crate) fn subspan<R: RangeBounds<usize>>(&self, range: R) -> Option<Span> {
        #[cfg(not(span_locations))]
        {
            let _ = range;
            None
        }

        #[cfg(span_locations)]
        {
            use core::ops::Bound;

            let lo = match range.start_bound() {
                Bound::Included(start) => {
                    let start = u32::try_from(*start).ok()?;
                    self.span.inner.lo.checked_add(start)?
                }
                Bound::Excluded(start) => {
                    let start = u32::try_from(*start).ok()?;
                    self.span.inner.lo.checked_add(start)?.checked_add(1)?
                }
                Bound::Unbounded => self.span.inner.lo,
            };
            let hi = match range.end_bound() {
                Bound::Included(end) => {
                    let end = u32::try_from(*end).ok()?;
                    self.span.inner.lo.checked_add(end)?.checked_add(1)?
                }
                Bound::Excluded(end) => {
                    let end = u32::try_from(*end).ok()?;
                    self.span.inner.lo.checked_add(end)?
                }
                Bound::Unbounded => self.span.inner.hi,
            };
            if lo <= hi && hi <= self.span.inner.hi {
                Some(Span {
                    lo,
                    hi,
                    source: self.span.inner.source.clone(),
                })
            } else {
                None
            }
        }
    }
}

impl Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.repr, f)
    }
}

impl Debug for Literal {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut debug = fmt.debug_struct("Literal");
        debug.field("lit", &format_args!("{}", self.repr));
        debug_span_field_if_nontrivial(&mut debug, &self.span.inner);
        debug.finish()
    }
}

fn escape_utf8(string: &str, repr: &mut String) {
    let mut chars = string.chars();
    while let Some(ch) = chars.next() {
        if ch == '\0' {
            repr.push_str(
                if chars
                    .as_str()
                    .starts_with(|next| '0' <= next && next <= '7')
                {
                    // circumvent clippy::octal_escapes lint
                    r"\x00"
                } else {
                    r"\0"
                },
            );
        } else if ch == '\'' {
            // escape_debug turns this into "\'" which is unnecessary.
            repr.push(ch);
        } else {
            repr.extend(ch.escape_debug());
        }
    }
}

#[cfg(feature = "proc-macro")]
pub(crate) trait FromStr2: FromStr<Err = proc_macro::LexError> {
    fn from_str_unchecked(src: &str) -> Self {
        Self::from_str(src).unwrap()
    }
}

#[cfg(feature = "proc-macro")]
impl FromStr2 for proc_macro::TokenStream {}

#[cfg(feature = "proc-macro")]
impl FromStr2 for proc_macro::Literal {}
