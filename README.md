# reqwest_resume

[![Crates.io](https://img.shields.io/crates/v/reqwest_resume.svg?style=flat-square&maxAge=86400)](https://crates.io/crates/reqwest_resume)
[![Apache-2.0 licensed](https://img.shields.io/crates/l/reqwest_resume.svg?style=flat-square&maxAge=2592000)](LICENSE.txt)
[![Build Status](https://ci.appveyor.com/api/projects/status/github/alecmocatta/reqwest_resume?branch=master&svg=true)](https://ci.appveyor.com/project/alecmocatta/reqwest-resume)
[![Build Status](https://circleci.com/gh/alecmocatta/reqwest_resume/tree/master.svg?style=shield)](https://circleci.com/gh/alecmocatta/reqwest_resume)
[![Build Status](https://travis-ci.com/alecmocatta/reqwest_resume.svg?branch=master)](https://travis-ci.com/alecmocatta/reqwest_resume)

[Docs](https://docs.rs/reqwest_resume/0.1.0)

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
Licensed under Apache License, Version 2.0, ([LICENSE.txt](LICENSE.txt) or http://www.apache.org/licenses/LICENSE-2.0).

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be licensed as above, without any additional terms or conditions.
