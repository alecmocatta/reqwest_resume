//! Wrapper that uses the `Range` HTTP header to resume get requests.
//!
//! <p style="font-family: 'Fira Sans',sans-serif;padding:0.3em 0"><strong>
//! <a href="https://crates.io/crates/reqwest_resume">📦&nbsp;&nbsp;Crates.io</a>&nbsp;&nbsp;│&nbsp;&nbsp;<a href="https://github.com/alecmocatta/reqwest_resume">📑&nbsp;&nbsp;GitHub</a>&nbsp;&nbsp;│&nbsp;&nbsp;<a href="https://constellation.zulipchat.com/#narrow/stream/213236-subprojects">💬&nbsp;&nbsp;Chat</a>
//! </strong></p>
//!
//! It's a thin wrapper around [`reqwest`](https://github.com/seanmonstar/reqwest). It's a work in progress – wrapping functionality is copied across on an as-needed basis. Feel free to open a PR/issue if you need something.
//!
//! # Example
//!
//! ```
//! use async_compression::futures::bufread::GzipDecoder;
//! use futures::{io::BufReader, AsyncBufReadExt, StreamExt, TryStreamExt};
//! use std::io;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let url = "http://commoncrawl.s3.amazonaws.com/crawl-data/CC-MAIN-2018-30/warc.paths.gz";
//! let body = reqwest_resume::get(url.parse().unwrap()).await.unwrap();
//! // Content-Encoding isn't set, so decode manually
//! let body = body
//!     .bytes_stream()
//!     .map_err(|e| io::Error::new(io::ErrorKind::Other, e));
//! let body = futures::io::BufReader::new(body.into_async_read());
//! let mut body = GzipDecoder::new(body); // Content-Encoding isn't set, so decode manually
//! body.multiple_members(true);
//!
//! let mut lines = BufReader::new(body).lines();
//! while let Some(line) = lines.next().await {
//!     println!("{}", line.unwrap());
//! }
//! # }
//! ```

#![doc(html_root_url = "https://docs.rs/reqwest_resume/0.3.0")]
#![warn(
	missing_copy_implementations,
	missing_debug_implementations,
	missing_docs,
	trivial_casts,
	trivial_numeric_casts,
	unused_import_braces,
	unused_qualifications,
	unused_results,
	clippy::pedantic
)] // from https://github.com/rust-unofficial/patterns/blob/master/anti_patterns/deny-warnings.md
#![allow(
	clippy::new_without_default,
	clippy::must_use_candidate,
	clippy::missing_errors_doc
)]

use bytes::Bytes;
use futures::{ready, Stream, TryFutureExt};
use log::trace;
use std::{
	future::Future, pin::Pin, task::{Context, Poll}
};

/// Extension to [`reqwest::Client`] that provides a method to convert it
pub trait ClientExt {
	/// Convert a [`reqwest::Client`] into a [`reqwest_resume::Client`](Client)
	fn resumable(self) -> Client;
}
impl ClientExt for reqwest::Client {
	fn resumable(self) -> Client {
		Client(self)
	}
}

/// A `Client` to make Requests with.
///
/// See [`reqwest::Client`].
#[derive(Debug)]
pub struct Client(reqwest::Client);
impl Client {
	/// Constructs a new `Client`.
	///
	/// See [`reqwest::Client::new()`].
	pub fn new() -> Self {
		Self(reqwest::Client::new())
	}
	/// Convenience method to make a `GET` request to a URL.
	///
	/// See [`reqwest::Client::get()`].
	pub fn get(&self, url: reqwest::Url) -> RequestBuilder {
		// <U: reqwest::IntoUrl>
		RequestBuilder(self.0.clone(), reqwest::Method::GET, url)
	}
}

/// A builder to construct the properties of a Request.
///
/// See [`reqwest::RequestBuilder`].
#[derive(Debug)]
pub struct RequestBuilder(reqwest::Client, reqwest::Method, reqwest::Url);
impl RequestBuilder {
	/// Constructs the Request and sends it the target URL, returning a Response.
	///
	/// See [`reqwest::RequestBuilder::send()`].
	pub fn send(&mut self) -> impl Future<Output = reqwest::Result<Response>> {
		let (client, method, url) = (self.0.clone(), self.1.clone(), self.2.clone());
		let builder = self.0.request(method.clone(), url.clone());
		async move {
			let response = builder.send().await?;
			let headers = hyperx::Headers::from(response.headers());
			let accept_byte_ranges =
				if let Some(&hyperx::header::AcceptRanges(ref ranges)) = headers.get() {
					ranges
						.iter()
						.any(|u| *u == hyperx::header::RangeUnit::Bytes)
				} else {
					false
				};
			Ok(Response {
				client,
				method,
				url,
				response,
				accept_byte_ranges,
				pos: 0,
			})
		}
	}
}

/// A Response to a submitted Request.
///
/// See [`reqwest::Response`].
#[derive(Debug)]
pub struct Response {
	client: reqwest::Client,
	method: reqwest::Method,
	url: reqwest::Url,
	response: reqwest::Response,
	accept_byte_ranges: bool,
	pos: u64,
}
impl Response {
	/// Convert the response into a `Stream` of `Bytes` from the body.
	///
	/// See [`reqwest::Response::bytes_stream()`].
	pub fn bytes_stream(self) -> impl Stream<Item = reqwest::Result<Bytes>> {
		Decoder {
			client: self.client,
			method: self.method,
			url: self.url,
			decoder: Box::pin(self.response.bytes_stream()),
			accept_byte_ranges: self.accept_byte_ranges,
			pos: self.pos,
		}
	}
}

struct Decoder {
	client: reqwest::Client,
	method: reqwest::Method,
	url: reqwest::Url,
	decoder: Pin<Box<dyn Stream<Item = reqwest::Result<Bytes>> + Send + Unpin>>,
	accept_byte_ranges: bool,
	pos: u64,
}
impl Stream for Decoder {
	type Item = reqwest::Result<Bytes>;

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
		loop {
			match ready!(self.decoder.as_mut().poll_next(cx)) {
				Some(Err(err)) => {
					if !self.accept_byte_ranges {
						// TODO: we could try, for those servers that don't output Accept-Ranges but work anyway
						trace!("couldn't resume HTTP request with error {:?}", err);
						break Poll::Ready(Some(Err(err)));
					}
					println!("resuming HTTP request due to error {:?}", err);
					let builder = self.client.request(self.method.clone(), self.url.clone());
					let mut headers = hyperx::Headers::new();
					headers.set(hyperx::header::Range::Bytes(vec![
						hyperx::header::ByteRangeSpec::AllFrom(self.pos),
					]));
					let builder = builder.headers(headers.into());
					// https://developer.mozilla.org/en-US/docs/Web/HTTP/Range_requests
					// https://github.com/sdroege/gst-plugin-rs/blob/dcb36832329fde0113a41b80ebdb5efd28ead68d/gst-plugin-http/src/httpsrc.rs
					self.decoder = Box::pin(
						builder
							.send()
							.map_ok(reqwest::Response::bytes_stream)
							.try_flatten_stream(),
					);
				}
				Some(Ok(n)) => {
					self.pos += n.len() as u64;
					break Poll::Ready(Some(Ok(n)));
				}
				None => break Poll::Ready(None),
			}
		}
	}
}

/// Shortcut method to quickly make a GET request.
///
/// See [`reqwest::get`].
pub fn get(url: reqwest::Url) -> impl Future<Output = reqwest::Result<Response>> {
	// <T: IntoUrl>
	Client::new().get(url).send()
}

#[cfg(test)]
mod test {
	use async_compression::futures::bufread::GzipDecoder; // TODO: use stream or https://github.com/alexcrichton/flate2-rs/pull/214
	use futures::{future::join_all, io::BufReader, AsyncBufReadExt, StreamExt, TryStreamExt};
	use std::io;

	#[tokio::test]
	#[ignore] // painful on CI. TODO
	async fn dl_s3() {
		// Requests to large files on S3 regularly time out or close when made from slower connections. This test is fairly meaningless from fast connections. TODO
		let body = reqwest::get(
			"http://commoncrawl.s3.amazonaws.com/crawl-data/CC-MAIN-2018-30/warc.paths.gz",
		)
		.await
		.unwrap();
		let body = body
			.bytes_stream()
			.map_err(|e| io::Error::new(io::ErrorKind::Other, e));
		let body = BufReader::new(body.into_async_read());
		let mut body = GzipDecoder::new(body); // Content-Encoding isn't set, so decode manually
		body.multiple_members(true);
		let handles = BufReader::new(body)
			.lines()
			.map(|url| format!("http://commoncrawl.s3.amazonaws.com/{}", url.unwrap()))
			.take(10)
			.map(|url| {
				tokio::spawn(async move {
					println!("{}", url);
					let body = super::get(url.parse().unwrap()).await.unwrap();
					let body = body
						.bytes_stream()
						.map_err(|e| io::Error::new(io::ErrorKind::Other, e));
					let body = BufReader::new(body.into_async_read());
					let mut body = GzipDecoder::new(body); // Content-Encoding isn't set, so decode manually
					body.multiple_members(true);
					let n = futures::io::copy(&mut body, &mut futures::io::sink())
						.await
						.unwrap();
					println!("{}", n);
				})
			})
			.collect::<Vec<_>>()
			.await;
		join_all(handles)
			.await
			.into_iter()
			.collect::<Result<(), _>>()
			.unwrap();
	}
}
