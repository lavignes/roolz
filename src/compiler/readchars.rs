use std::{
    error,
    fmt::{self, Display, Formatter},
    io::{self, Bytes, Read},
    str,
};

#[derive(Debug)]
pub enum Error {
    IoError(io::Error),
    Utf8Error(str::Utf8Error),
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
            Error::IoError(err) => Some(err),
            Error::Utf8Error(err) => Some(err),
        }
    }
}

impl From<io::Error> for Error {
    #[inline]
    fn from(err: io::Error) -> Error {
        Self::IoError(err)
    }
}

impl From<str::Utf8Error> for Error {
    #[inline]
    fn from(err: str::Utf8Error) -> Error {
        Self::Utf8Error(err)
    }
}

pub struct ReadChars<R> {
    inner: Bytes<R>,
}

impl<R: Read> From<R> for ReadChars<R> {
    #[inline]
    fn from(r: R) -> ReadChars<R> {
        Self { inner: r.bytes() }
    }
}

impl<R: Read> Iterator for ReadChars<R> {
    type Item = Result<char, Error>;

    fn next(&mut self) -> Option<Result<char, Error>> {
        let mut bytes = [0, 0, 0, 0];
        for i in 0..bytes.len() as usize {
            let result = self.inner.next()?;
            if let Err(err) = result {
                return Some(Err(err.into()));
            }
            bytes[i] = result.unwrap();

            let result = str::from_utf8(&bytes[..i + 1]);
            if let Ok(s) = result {
                return Some(Ok(s.chars().next().unwrap()));
            }
        }
        // We didn't find a character, so return the utf-8 error we would have thrown
        Some(Err(str::from_utf8(&bytes)
            .map_err(Error::from)
            .err()
            .unwrap()))
    }
}
