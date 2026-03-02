#[cfg(feature = "std")]
extern crate std;

extern crate alloc;

#[cfg(feature = "std")]
use std::{io, num::NonZeroU64};

use core::{error, fmt};

use alloc::string::String;

#[derive(Debug, Clone)]
#[cfg(feature = "std")]
pub struct Line {
    pub message: String,
    pub line_number: Option<NonZeroU64>,
}

#[cfg(feature = "std")]
impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.line_number {
            Some(ln) => write!(f, "Line {}: {}", ln, self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

#[cfg(feature = "std")]
impl error::Error for Line {}

#[derive(Debug)]
#[cfg(feature = "std")]
pub enum TextParse {
    Io(io::Error),
    Lexer(Line),
    Parser(Line),
}

#[cfg(feature = "std")]
impl TextParse {
    pub fn from_lexer(message: String, line_number: NonZeroU64) -> TextParse {
        TextParse::Lexer(Line {
            message,
            line_number: Some(line_number),
        })
    }

    pub fn from_parser(message: String, line_number: NonZeroU64) -> TextParse {
        TextParse::Parser(Line {
            message,
            line_number: Some(line_number),
        })
    }

    pub fn eof() -> TextParse {
        TextParse::Parser(Line {
            message: String::from("Unexpected end-of-file"),
            line_number: None,
        })
    }
}

#[cfg(feature = "std")]
impl From<io::Error> for TextParse {
    fn from(err: io::Error) -> Self {
        TextParse::Io(err)
    }
}

#[cfg(feature = "std")]
impl fmt::Display for TextParse {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "{}", msg),
            Self::Lexer(err) => write!(f, "{}", err),
            Self::Parser(err) => write!(f, "{}", err),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TextParse {}

#[derive(Debug)]
#[cfg(feature = "std")]
pub enum Write {
    Validation(String),
    Io(std::io::Error),
}

#[cfg(feature = "std")]
impl From<io::Error> for Write {
    fn from(err: io::Error) -> Self {
        Write::Io(err)
    }
}

#[cfg(feature = "std")]
impl From<Validation> for Write {
    fn from(val_err: Validation) -> Self {
        Self::Validation(val_err.0)
    }
}

#[cfg(feature = "std")]
impl fmt::Display for Write {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Validation(msg) => write!(f, "{}", msg),
            Self::Io(err) => write!(f, "{}", err),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Write {}

#[derive(Debug, PartialEq, Eq)]
pub struct Validation(pub String);

impl From<String> for Validation {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for Validation {
    fn from(s: &str) -> Self {
        Self(String::from(s))
    }
}

impl fmt::Display for Validation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl error::Error for Validation {}

/// Return type for validating writability of entities and other items
pub type ValidationResult = Result<(), Validation>;

#[cfg(feature = "std")]
pub type TextParseResult<T> = Result<T, TextParse>;

#[cfg(feature = "std")]
pub type WriteResult = Result<(), Write>;
