use std::{fmt, mem};

use futures::{Stream, Poll, Async};
use bytes::{Buf, Bytes};
use hyper::body::Payload;

/// An asynchronous `Stream`.
pub struct Body {
    inner: Inner,
}

enum Inner {
    Reusable(Bytes),
    Hyper(::hyper::Body),
}

impl Body {
    fn poll_inner(&mut self) -> &mut ::hyper::Body {
        match self.inner {
            Inner::Hyper(ref mut body) => return body,
            Inner::Reusable(_) => (),
        }

        let bytes = match mem::replace(&mut self.inner, Inner::Reusable(Bytes::new())) {
            Inner::Reusable(bytes) => bytes,
            Inner::Hyper(_) => unreachable!(),
        };

        self.inner = Inner::Hyper(bytes.into());

        match self.inner {
            Inner::Hyper(ref mut body) => return body,
            Inner::Reusable(_) => unreachable!(),
        }
    }

    pub(crate) fn content_length(&self) -> Option<u64> {
        match self.inner {
            Inner::Reusable(ref bytes) => Some(bytes.len() as u64),
            Inner::Hyper(ref body) => body.content_length(),
        }
    }

    #[inline]
    pub(crate) fn wrap(body: ::hyper::Body) -> Body {
        Body {
            inner: Inner::Hyper(body),
        }
    }

    #[inline]
    pub(crate) fn empty() -> Body {
        Body {
            inner: Inner::Hyper(::hyper::Body::empty()),
        }
    }

    #[inline]
    pub(crate) fn reusable(chunk: Bytes) -> Body {
        Body {
            inner: Inner::Reusable(chunk),
        }
    }

    #[inline]
    pub(crate) fn into_hyper(self) -> (Option<Bytes>, ::hyper::Body) {
        match self.inner {
            Inner::Reusable(chunk) => (Some(chunk.clone()), chunk.into()),
            Inner::Hyper(b) => (None, b),
        }
    }
}

impl Stream for Body {
    type Item = Chunk;
    type Error = ::Error;

    #[inline]
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match try_!(self.poll_inner().poll()) {
            Async::Ready(opt) => Ok(Async::Ready(opt.map(|chunk| Chunk {
                inner: chunk,
            }))),
            Async::NotReady => Ok(Async::NotReady),
        }
    }
}

impl From<::hyper::Body> for Body {
    fn from(body: ::hyper::Body) -> Self {
        Body::wrap(body)
    }
}

impl From<Bytes> for Body {
    #[inline]
    fn from(bytes: Bytes) -> Body {
        Body::reusable(bytes)
    }
}

impl From<Vec<u8>> for Body {
    #[inline]
    fn from(vec: Vec<u8>) -> Body {
        Body::reusable(vec.into())
    }
}

impl From<&'static [u8]> for Body {
    #[inline]
    fn from(s: &'static [u8]) -> Body {
        Body::reusable(Bytes::from_static(s))
    }
}

impl From<String> for Body {
    #[inline]
    fn from(s: String) -> Body {
        Body::reusable(s.into())
    }
}

impl From<&'static str> for Body {
    #[inline]
    fn from(s: &'static str) -> Body {
        s.as_bytes().into()
    }
}

/// A chunk of bytes for a `Body`.
///
/// A `Chunk` can be treated like `&[u8]`.
#[derive(Default)]
pub struct Chunk {
    inner: ::hyper::Chunk,
}

impl Chunk {
    #[inline]
    pub(crate) fn from_chunk(chunk: Bytes) -> Chunk {
        Chunk {
            inner: ::hyper::Chunk::from(chunk)
        }
    }
}
impl Buf for Chunk {
    fn bytes(&self) -> &[u8] {
        self.inner.bytes()
    }

    fn remaining(&self) -> usize {
        self.inner.remaining()
    }

    fn advance(&mut self, n: usize) {
        self.inner.advance(n);
    }
}

impl AsRef<[u8]> for Chunk {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &*self
    }
}

impl ::std::ops::Deref for Chunk {
    type Target = [u8];
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.inner.as_ref()
    }
}

impl Extend<u8> for Chunk {
    fn extend<T>(&mut self, iter: T)
    where T: IntoIterator<Item=u8> {
        self.inner.extend(iter)
    }
}

impl IntoIterator for Chunk {
    type Item = u8;
    //XXX: exposing type from hyper!
    type IntoIter = <::hyper::Chunk as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

impl fmt::Debug for Body {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Body")
            .finish()
    }
}

impl fmt::Debug for Chunk {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}
