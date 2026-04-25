//! Cursor pagination wrapper.
//!
//! The GraphANN server returns `next_cursor` on the few endpoints that
//! support cursor pagination. Page-aware methods on the SDK return a
//! [`PageStream`] which implements `futures::Stream`, so callers can use
//! `try_next` / `for_each` without manually threading the cursor.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;
use serde::de::DeserializeOwned;

use crate::error::Error;

/// One page of results from a cursor-paginated endpoint.
#[derive(Debug, Clone)]
pub struct Page<T> {
    /// Items in this page.
    pub items: Vec<T>,
    /// Cursor for the next page (absent when exhausted).
    pub next_cursor: Option<String>,
}

/// Closure invoked to fetch one page given an optional cursor.
type Fetcher<T> = Box<
    dyn FnMut(Option<String>) -> futures::future::BoxFuture<'static, Result<Page<T>, Error>> + Send,
>;

/// Async stream of pages.
///
/// Drop the stream to stop polling early.
pub struct PageStream<T> {
    state: StreamState<T>,
}

enum StreamState<T> {
    Idle {
        fetcher: Fetcher<T>,
        next_cursor: Option<String>,
        first_call: bool,
    },
    Pending(
        futures::future::BoxFuture<'static, Result<Page<T>, Error>>,
        Fetcher<T>,
    ),
    Done,
}

impl<T> PageStream<T>
where
    T: DeserializeOwned + Send + 'static,
{
    pub(crate) fn new<F, Fut>(mut fetcher: F) -> Self
    where
        F: FnMut(Option<String>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<Page<T>, Error>> + Send + 'static,
    {
        let boxed: Fetcher<T> = Box::new(move |cursor| Box::pin(fetcher(cursor)));
        Self {
            state: StreamState::Idle {
                fetcher: boxed,
                next_cursor: None,
                first_call: true,
            },
        }
    }
}

impl<T> Stream for PageStream<T>
where
    T: DeserializeOwned + Send + 'static + Unpin,
{
    type Item = Result<Page<T>, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            // Drive the in-flight future.
            let owned = std::mem::replace(&mut this.state, StreamState::Done);
            match owned {
                StreamState::Idle {
                    mut fetcher,
                    next_cursor,
                    first_call,
                } => {
                    if !first_call && next_cursor.is_none() {
                        // Cursor exhausted — stream completes.
                        this.state = StreamState::Done;
                        return Poll::Ready(None);
                    }
                    let fut = fetcher(next_cursor);
                    this.state = StreamState::Pending(fut, fetcher);
                }
                StreamState::Pending(mut fut, fetcher) => match fut.as_mut().poll(cx) {
                    Poll::Pending => {
                        this.state = StreamState::Pending(fut, fetcher);
                        return Poll::Pending;
                    }
                    Poll::Ready(Ok(page)) => {
                        let next_cursor = page.next_cursor.clone();
                        this.state = StreamState::Idle {
                            fetcher,
                            next_cursor,
                            first_call: false,
                        };
                        return Poll::Ready(Some(Ok(page)));
                    }
                    Poll::Ready(Err(err)) => {
                        this.state = StreamState::Done;
                        return Poll::Ready(Some(Err(err)));
                    }
                },
                StreamState::Done => {
                    this.state = StreamState::Done;
                    return Poll::Ready(None);
                }
            }
        }
    }
}
