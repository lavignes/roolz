use std::{
    convert::From,
    error,
    fmt::{self, Display, Formatter},
    io::{self, ErrorKind, Read},
    result,
};

use crate::compiler::readchars::{self, ReadChars};

#[derive(Debug, Copy, Clone)]
enum State {
    Comment,

    Root,
    PkgP,
    PkgK,
    PkgG,

    PkgIdentifier,
    OptionalPkgNameDot,

    Semi,
    //    Name,
    //    Identifier,
    //    Semi,
    //    Dot,
    //    Not,
    //    And,
    //    Or,
    //    Xor,
    //    Eq,
    //    NewLine,
    //    BraceOpen,
    //    BraceClose,
    //    ParenOpen,
    //    ParenClose,
    //    Rule,
    //    Fact,
    //    Thus,
    //    Value,
}

#[derive(Debug)]
pub enum Error {
    IoError((usize, usize), io::Error),
    ParseError((usize, usize), String),
    InvalidState((usize, usize)),
}

impl Display for Error {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl error::Error for Error {
    #[inline]
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Error::IoError(_, err) => Some(err),
            _ => None,
        }
    }
}

pub type Result<T> = result::Result<T, Error>;

pub struct Parser {
    line: usize,
    column: usize,
    state_stack: Vec<State>,
    string_buffer: String,
}

impl Parser {
    fn new() -> Self {
        Parser {
            line: 1,
            column: 0,
            state_stack: vec![
                State::Root,
                State::Semi,
                State::OptionalPkgNameDot,
                State::PkgIdentifier,
                State::PkgG,
                State::PkgK,
                State::PkgP,
            ],
            string_buffer: String::with_capacity(32),
        }
    }

    pub fn parse<R: Read>(reader: R) -> Result<()> {
        let mut parser = Self::new();
        parser.parse_now(reader)
    }

    fn parse_now<R: Read>(&mut self, reader: R) -> Result<()> {
        let chars = ReadChars::from(reader);
        'next_char: for result in chars {
            self.column += 1;
            let c = result.map_err(|err| {
                Error::IoError(
                    (self.line, self.column),
                    match err {
                        readchars::Error::IoError(err) => err,
                        readchars::Error::Utf8Error(err) => {
                            io::Error::new(ErrorKind::InvalidData, err)
                        }
                    },
                )
            })?;

            'check_char: loop {
                if c.is_whitespace() {
                    // Update the parser location
                    if c == '\n' {
                        self.column = 1;
                        self.line += 1;
                        // Also if we're in a comment we can exit when we see a newline
                        if let State::Comment = self.peek()? {
                            self.pop()?;
                        }
                    }
                    continue 'next_char;
                }
                match self.peek()? {
                    State::Comment => { /* ignored */ }
                    State::PkgP => {
                        if self.transitions_to_comment(c) {
                            continue 'next_char;
                        } else if c == 'p' {
                            self.pop()?;
                        } else {
                            return self.parse_err(format!(
                                "Unexpected input: \"{}\", expecting start of package (ex: \"pkg my.package.name;\")",
                                c
                            ));
                        }
                    }
                    State::PkgK => {
                        if c == 'k' {
                            self.pop()?;
                        } else {
                            return self.parse_err(format!(
                                "Unexpected input: \"p{}\", expecting start of package (ex: \"pkg my.package.name;\")",
                                c
                            ));
                        }
                    }
                    State::PkgG => {
                        if c == 'g' {
                            self.pop()?;
                        } else {
                            return self.parse_err(format!(
                                "Unexpected input: \"pk{}\", expecting start of package (ex: \"pkg my.package.name;\")",
                                c
                            ));
                        }
                    }
                    State::PkgIdentifier => {
                        if self.string_buffer.is_empty() {
                            if Self::is_identifier_start(c) {
                                self.string_buffer.push(c);
                            } else {
                                return self.parse_err(format!(
                                    "Unexpected input: \"{}\", expecting identifier",
                                    c
                                ));
                            }
                        } else {
                            if c.is_alphanumeric() {
                                self.string_buffer.push(c);
                            } else {
                                self.pop()?;
                                continue 'check_char;
                            }
                        }
                    }
                    State::OptionalPkgNameDot => {
                        // We see another dot, so there must be another identifier
                        if c == '.' {
                            self.string_buffer.push(c);
                            self.push(State::PkgIdentifier);
                        } else {
                            self.pop()?;
                            continue 'check_char;
                        }
                    }
                    State::Semi => {
                        if c == ';' {
                            self.pop()?;
                        } else {
                            return self.parse_err(format!(
                                "Unexpected input: \"{}\", expecting \";\"",
                                c
                            ));
                        }
                    }
                    _ => unimplemented!(),
                }
                continue 'next_char;
            }
        }
        if let State::Root = self.pop()? {
            Ok(())
        } else {
            self.parse_err(format!("Unexpected end of input"))
        }
    }

    #[inline]
    fn parse_err(&self, msg: String) -> Result<()> {
        Err(Error::ParseError((self.line, self.column), msg))
    }

    #[inline]
    fn transitions_to_comment(&mut self, c: char) -> bool {
        if c == '#' {
            self.push(State::Comment);
            true
        } else {
            false
        }
    }

    #[inline]
    fn is_identifier_start(c: char) -> bool {
        c.is_alphabetic() || c == '_'
    }

    #[inline]
    fn push(&mut self, state: State) {
        self.state_stack.push(state)
    }

    #[inline]
    fn pop(&mut self) -> Result<State> {
        self.state_stack
            .pop()
            .ok_or(Error::InvalidState((self.line, self.column)))
    }

    #[inline]
    fn peek(&self) -> Result<State> {
        self.state_stack
            .last()
            .map(|state| *state)
            .ok_or(Error::InvalidState((self.line, self.column)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test() {
        let pkg = r#"
            # hello
            pkg hello;
        "#;
        Parser::parse(Cursor::new(pkg)).unwrap();
    }
}
