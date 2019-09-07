# reqwest_resume

[![Crates.io](https://img.shields.io/crates/v/reqwest_resume.svg?maxAge=86400)](https://crates.io/crates/reqwest_resume)
[![MIT / Apache 2.0 licensed](https://img.shields.io/crates/l/reqwest_resume.svg?maxAge=2592000)](#License)
[![Build Status](https://dev.azure.com/alecmocatta/reqwest_resume/_apis/build/status/tests?branchName=master)](https://dev.azure.com/alecmocatta/reqwest_resume/_build/latest?branchName=master)

[Docs](https://docs.rs/reqwest_resume/0.2.1)

Wrapper that uses the `Range` HTTP header to resume get requests.

It's a thin wrapper around [`reqwest`](https://github.com/seanmonstar/reqwest). It's a work in progress – wrapping functionality is copied across on an as-needed basis. Feel free to open a PR/issue if you need something.

## Example

```rust
extern crate reqwest_resume;
extern crate flate2;

use std::io::{BufRead, BufReader};

let url = "http://commoncrawl.s3.amazonaws.com/crawl-data/CC-MAIN-2018-30/warc.paths.gz";
let body = reqwest_resume::get(url.parse().unwrap()).unwrap();
// Content-Encoding isn't set, so decode manually
let body = flate2::read::MultiGzDecoder::new(body);

for line in BufReader::new(body).lines() {
	println!("{}", line.unwrap());
}
```

## License
Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE.txt](LICENSE-APACHE.txt) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT.txt](LICENSE-MIT.txt) or http://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
