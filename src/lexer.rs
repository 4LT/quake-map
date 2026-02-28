#[cfg(feature = "std")]
extern crate std;

use std::{
    cell::Cell,
    convert::TryInto,
    ffi::CString,
    fmt, io,
    iter::once,
    mem::transmute,
    num::{NonZeroU64, NonZeroU8},
    string::String,
    vec::Vec,
};

use fmt::{Display, Formatter};

use crate::error;

pub type LexResult = Result<Option<LineToken>, Cell<Option<error::TextParse>>>;

const TEXT_CAPACITY: usize = 32;

#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Token {
    OpenParen,
    CloseParen,
    OpenCurly,
    CloseCurly,
    OpenSquare,
    CloseSquare,
    QuotedString(Vec<NonZeroU8>),
    BareString(Vec<NonZeroU8>),
}

impl Token {
    pub fn as_number_text(&self) -> &str {
        match &self {
            Token::BareString(s) => {
                let slice = unsafe { transmute::<&[NonZeroU8], &[u8]>(&s[..]) };
                str::from_utf8(slice).unwrap_or("")
            }
            _ => "",
        }
    }

    pub fn to_string_fast(&self) -> String {
        match &self {
            Token::OpenParen => String::from("("),
            Token::CloseParen => String::from(")"),
            Token::OpenCurly => String::from("{"),
            Token::CloseCurly => String::from("}"),
            Token::OpenSquare => String::from("["),
            Token::CloseSquare => String::from("]"),
            Token::QuotedString(s) => once('"')
                .chain(s.iter().map(|ch| char::from(ch.get())).chain(once('"')))
                .collect(),
            Token::BareString(s) => {
                s.iter().map(|ch| char::from(ch.get())).collect()
            }
        }
    }
}

impl Display for Token {
    fn fmt(&self, fmtr: &mut Formatter) -> fmt::Result {
        match &self {
            Token::OpenParen => fmtr.write_str("("),
            Token::CloseParen => fmtr.write_str(")"),
            Token::OpenCurly => fmtr.write_str("{"),
            Token::CloseCurly => fmtr.write_str("}"),
            Token::OpenSquare => fmtr.write_str("["),
            Token::CloseSquare => fmtr.write_str("]"),
            _ => fmtr.write_str(&self.to_string_fast()),
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct LineToken {
    pub token: Token,
    pub line_number: NonZeroU64,
}

impl LineToken {
    pub fn from_text(text: Vec<NonZeroU8>, line_number: NonZeroU64) -> Self {
        let token = if text.len() == 1 {
            match text[0].get() {
                b'(' => Token::OpenParen,
                b')' => Token::CloseParen,
                b'{' => Token::OpenCurly,
                b'}' => Token::CloseCurly,
                b'[' => Token::OpenSquare,
                b']' => Token::CloseSquare,
                _ => Token::BareString(text),
            }
        } else if text.len() >= 2
            && text[0].get() == b'"'
            && text.last() == Some(&b'"'.try_into().unwrap())
        {
            Token::QuotedString(text[1..text.len() - 1].to_vec())
        } else {
            Token::BareString(text)
        };

        Self { token, line_number }
    }

    pub fn into_bare_cstring(self) -> CString {
        match self.token {
            Token::BareString(s) | Token::QuotedString(s) => s.into(),
            _ => CString::new("").unwrap(),
        }
    }

    pub fn is_quoted(&self) -> bool {
        matches!(&self.token, Token::QuotedString(_))
    }
}

impl fmt::Display for LineToken {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: line {}", self.token, self.line_number)
    }
}

pub struct TokenIterator<R: io::Read> {
    text: Cell<Option<Vec<NonZeroU8>>>,
    state: fn(iter: &mut TokenIterator<R>) -> Option<LineToken>,
    byte: Option<NonZeroU8>,
    last_byte: Option<NonZeroU8>,
    line_number: NonZeroU64,
    input: io::Bytes<R>,
}

impl<R: io::Read> TokenIterator<R> {
    #[allow(
        clippy::unbuffered_bytes,
        reason = "Read implementation expected to be buffered"
    )]
    pub fn new(reader: R) -> TokenIterator<R> {
        TokenIterator {
            text: Cell::new(None),
            state: lex_default,
            byte: None,
            last_byte: None,
            line_number: NonZeroU64::new(1).unwrap(),
            input: reader.bytes(),
        }
    }

    fn byte_read(&mut self, b: io::Result<u8>) -> LexResult {
        let byte = b.map_err(|e| Cell::new(Some(e.into())))?;

        self.byte = Some(
            byte.try_into()
                .map_err(|_| {
                    error::TextParse::from_lexer(
                        String::from("Null byte"),
                        self.line_number,
                    )
                })
                .map_err(Some)
                .map_err(Cell::new)?,
        );

        let maybe_token = (self.state)(self);

        if self.byte == NonZeroU8::new(b'\n')
            || self.last_byte == NonZeroU8::new(b'\r')
        {
            let next_line = self.line_number.get().saturating_add(1);
            unsafe {
                self.line_number = NonZeroU64::new_unchecked(next_line);
            }
        }

        self.last_byte = self.byte;

        Ok(maybe_token)
    }

    fn eof_read(&mut self) -> LexResult {
        if let Some(last_text) = self.text.replace(None) {
            if last_text[0] == NonZeroU8::new(b'"').unwrap()
                && (last_text.last() != NonZeroU8::new(b'"').as_ref()
                    || last_text.len() == 1)
            {
                Err(Cell::new(Some(error::TextParse::from_lexer(
                    String::from("Missing closing quote"),
                    self.line_number,
                ))))
            } else {
                Ok(Some(LineToken::from_text(last_text, self.line_number)))
            }
        } else {
            Ok(None)
        }
    }
}

impl<R: io::Read> Iterator for TokenIterator<R> {
    type Item = Result<LineToken, Cell<Option<error::TextParse>>>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(b) = self.input.next() {
                if let token @ Some(_) = self.byte_read(b).transpose() {
                    break token;
                }
            } else {
                break self.eof_read().transpose();
            }
        }
    }
}

fn lex_default<R: io::Read>(
    iterator: &mut TokenIterator<R>,
) -> Option<LineToken> {
    if !iterator.byte.unwrap().get().is_ascii_whitespace() {
        if iterator.byte == NonZeroU8::new(b'"') {
            iterator.state = lex_quoted;
            let mut text_bytes = Vec::with_capacity(TEXT_CAPACITY);
            text_bytes.push(iterator.byte.unwrap());
            iterator.text.replace(Some(text_bytes));
        } else if iterator.byte == NonZeroU8::new(b'/') {
            iterator.state = lex_maybe_comment;
        } else {
            iterator.state = lex_unquoted;
            let mut text_bytes = Vec::with_capacity(TEXT_CAPACITY);
            text_bytes.push(iterator.byte.unwrap());
            iterator.text.replace(Some(text_bytes));
        }
    }

    None
}

fn lex_comment<R: io::Read>(
    iterator: &mut TokenIterator<R>,
) -> Option<LineToken> {
    if iterator.byte == NonZeroU8::new(b'\r')
        || iterator.byte == NonZeroU8::new(b'\n')
    {
        iterator.state = lex_default;
    }

    None
}

fn lex_maybe_comment<R: io::Read>(
    iterator: &mut TokenIterator<R>,
) -> Option<LineToken> {
    if iterator.byte == NonZeroU8::new(b'/') {
        iterator.state = lex_comment;
    } else {
        let mut text_bytes: Vec<NonZeroU8> = Vec::with_capacity(TEXT_CAPACITY);
        text_bytes.push(NonZeroU8::new(b'/').unwrap());
        text_bytes.push(iterator.byte.unwrap());
        iterator.text.replace(Some(text_bytes));
        iterator.state = lex_unquoted;
    }

    None
}

fn lex_quoted<R: io::Read>(
    iterator: &mut TokenIterator<R>,
) -> Option<LineToken> {
    iterator
        .text
        .get_mut()
        .as_mut()
        .unwrap()
        .push(iterator.byte.unwrap());
    if iterator.byte == NonZeroU8::new(b'"') {
        let local_text = iterator.text.replace(None).unwrap();
        iterator.state = lex_default;

        Some(LineToken::from_text(local_text, iterator.line_number))
    } else {
        None
    }
}

fn lex_unquoted<R: io::Read>(
    iterator: &mut TokenIterator<R>,
) -> Option<LineToken> {
    if iterator.byte.unwrap().get().is_ascii_whitespace() {
        let local_text = iterator.text.replace(None).unwrap();
        iterator.state = lex_default;

        Some(LineToken::from_text(local_text, iterator.line_number))
    } else {
        iterator
            .text
            .get_mut()
            .as_mut()
            .unwrap()
            .push(iterator.byte.unwrap());

        None
    }
}
