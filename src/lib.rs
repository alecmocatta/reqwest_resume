//! Wrapper that uses the `Range` HTTP header to resume get requests.
//!
//! **[Crates.io](https://crates.io/crates/reqwest_resume) │ [Repo](https://github.com/alecmocatta/reqwest_resume)**
//!
//! It's a thin wrapper around [`reqwest`](https://github.com/seanmonstar/reqwest). It's a work in progress – wrapping functionality is copied across on an as-needed basis. Feel free to open a PR/issue if you need something.
//!
//! # Example
//!
//! ```
//! extern crate reqwest_resume;
//! extern crate flate2;
//!
//! use std::io::{BufRead, BufReader};
//!
//! let url = "http://commoncrawl.s3.amazonaws.com/crawl-data/CC-MAIN-2018-30/warc.paths.gz";
//! let body = reqwest_resume::get(url.parse().unwrap()).unwrap();
//! // Content-Encoding isn't set, so decode manually
//! let body = flate2::read::MultiGzDecoder::new(body);
//!
//! for line in BufReader::new(body).lines() {
//! 	println!("{}", line.unwrap());
//! }
//! ```

#![doc(html_root_url = "https://docs.rs/reqwest_resume/0.1.0")]
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
#![allow(clippy::new_without_default)]

use log::trace;
use std::io;

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
		RequestBuilder(self.0.clone(), reqwest::Method::Get, url)
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
	pub fn send(&mut self) -> reqwest::Result<Response> {
		let mut builder = self.0.request(self.1.clone(), self.2.clone());
		Ok(Response(
			self.0.clone(),
			self.1.clone(),
			self.2.clone(),
			builder.send()?,
			0,
		))
	}
}

/// A Response to a submitted Request.
///
/// See [`reqwest::Response`].
#[derive(Debug)]
pub struct Response(
	reqwest::Client,
	reqwest::Method,
	reqwest::Url,
	reqwest::Response,
	u64,
);
impl Response {}
impl io::Read for Response {
	fn read(&mut self, buf: &mut [u8]) -> Result<usize, io::Error> {
		loop {
			match self.3.read(buf) {
				Ok(n) => {
					self.4 += n as u64;
					break Ok(n);
				}
				Err(err) => {
					let accept_byte_ranges =
						if let Some(&reqwest::header::AcceptRanges(ref ranges)) =
							self.3.headers().get()
						{
							ranges
								.iter()
								.any(|u| *u == reqwest::header::RangeUnit::Bytes)
						} else {
							false
						};
					if accept_byte_ranges {
						trace!("resuming HTTP request due to error {:?}", err);
						let mut builder = self.0.request(self.1.clone(), self.2.clone());
						let _ = builder.header(reqwest::header::Range::Bytes(vec![
							reqwest::header::ByteRangeSpec::AllFrom(self.4),
						]));
						// https://developer.mozilla.org/en-US/docs/Web/HTTP/Range_requests
						// https://github.com/sdroege/gst-plugin-rs/blob/dcb36832329fde0113a41b80ebdb5efd28ead68d/gst-plugin-http/src/httpsrc.rs
						if let Ok(response) = builder.send() {
							self.3 = response;
							continue;
						}
					} else {
						// TODO: we could try, for those servers that don't output Accept-Ranges but work anyway
						trace!("couldn't resume HTTP request with error {:?}", err);
					}
					break Err(err);
				}
			}
		}
	}
}

/// Shortcut method to quickly make a GET request.
///
/// See [`reqwest::get`].
pub fn get(url: reqwest::Url) -> reqwest::Result<Response> {
	// <T: IntoUrl>
	Client::new().get(url).send()
}

#[cfg(test)]
mod test {
	use flate2;
	use reqwest;
	use std::{
		io::{self, BufRead, BufReader}, thread
	};

	#[test]
	#[ignore] // painful on CI. TODO
	fn dl_s3() {
		// Requests to large files on S3 regularly time out or close when made from slower connections. This test is fairly meaningless from fast connections. TODO
		let body = reqwest::get(
			"http://commoncrawl.s3.amazonaws.com/crawl-data/CC-MAIN-2018-30/warc.paths.gz",
		)
		.unwrap();
		let body = flate2::read::MultiGzDecoder::new(body); // Content-Encoding isn't set, so decode manually
		let handles = BufReader::new(body)
			.lines()
			.map(|url| format!("http://commoncrawl.s3.amazonaws.com/{}", url.unwrap()))
			.take(10)
			.map(|url| {
				println!("{}", url);
				thread::spawn(move || {
					// let body = reqwest::ClientBuilder::new().timeout(time::Duration::new(120,0)).build().unwrap().resumable().get(url.parse().unwrap()).send().unwrap();
					let body = super::get(url.parse().unwrap()).unwrap();
					let mut body = flate2::read::MultiGzDecoder::new(body);
					let n = io::copy(&mut body, &mut io::sink()).unwrap();
					println!("{}", n);
				})
			})
			.collect::<Vec<_>>();
		for handle in handles {
			handle.join().unwrap();
		}
	}
}
