use log::info;
use reqwest::header::{COOKIE, USER_AGENT};
use reqwest::{ClientBuilder, RequestBuilder, Response};
use std::path::Path;
use url::Url;

mod error {
    error_chain::error_chain! {}
}

use error::*;

// Copied from GNOME Epiphany-3.32.4.
fn user_agent() -> &'static str {
    return "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
            AppleWebkit/537.36 (KHTML, like Gecko) \
            Chrome/74.0.3729.169 \
            Safari/537.36";
}

pub struct Codeforces {
    pub server_url: Url,
    pub identy: String,
    pub contest_path: String,
    pub user_agent: String,
    pub prefer_cxx: String,
    pub prefer_py: String,
    pub retry_limit: i64,
    pub no_cookie: bool,
    pub cookie: String,
    client: reqwest::Client,
}

impl Codeforces {
    pub fn new(b: ClientBuilder) -> Result<Self> {
        let cf = Codeforces {
            server_url: Url::parse("https://codeforces.com").unwrap(),
            identy: String::from(""),
            contest_path: String::from(""),
            user_agent: String::from(user_agent()),
            prefer_cxx: String::from("c++17"),
            prefer_py: String::from("py3"),
            retry_limit: 3,
            no_cookie: false,
            cookie: String::from(""),
            client: b.build().chain_err(|| "can not build HTTP client")?,
        };
        Ok(cf)
    }

    // Override some config options from JSON config file.
    pub fn from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        use std::fs::File;
        use std::io::BufReader;
        let file = File::open(path).chain_err(|| "can not open file")?;
        let rdr = BufReader::new(file);

        use serde_json::Value;
        let v: Value = serde_json::from_reader(rdr).chain_err(|| "can not parse JSON")?;

        // Stupid code.  Maybe need some refactoring.
        match &v["server_url"] {
            Value::String(s) => {
                let u = Url::parse(s).chain_err(|| "can not parse url")?;
                self.server_url = u;
            }
            _ => (),
        };

        match &v["identy"] {
            Value::String(s) => self.identy = s.to_string(),
            _ => (),
        };

        match &v["contest_path"] {
            Value::String(s) => self.contest_path = s.to_string(),
            _ => (),
        };

        match &v["user_agent"] {
            Value::String(s) => self.user_agent = s.to_string(),
            _ => (),
        };

        match &v["prefer_cxx"] {
            Value::String(s) => self.prefer_cxx = s.to_string(),
            _ => (),
        };

        match &v["prefer_py"] {
            Value::String(s) => self.prefer_py = s.to_string(),
            _ => (),
        }

        match &v["retry_limit"] {
            Value::Number(n) => {
                if n.is_i64() {
                    self.retry_limit = n.as_i64().unwrap();
                }
            }
            _ => (),
        }

        match &v["no_cookie"] {
            Value::Bool(b) => self.no_cookie = *b,
            _ => (),
        };

        Ok(())
    }

    pub fn http_request_retry<F: Fn() -> RequestBuilder>(&self, req: F) -> Result<Response> {
        let mut retry_limit = self.retry_limit;
        loop {
            let resp = req().send();
            match &resp {
                Err(e) => {
                    if e.is_timeout() && retry_limit > 0 {
                        retry_limit -= 1;
                        info!("timeout, retrying");
                        continue;
                    } else {
                        return resp.chain_err(|| "http request failed");
                    }
                }
                _ => return Ok(resp.unwrap()),
            };
        }
    }

    fn add_header(&self, b: RequestBuilder) -> RequestBuilder {
        b.header(USER_AGENT, &self.user_agent)
            .header(COOKIE, &self.cookie)
    }

    pub fn get<P: AsRef<str>>(&self, p: P) -> Result<RequestBuilder> {
        let u = self
            .server_url
            .join(p.as_ref())
            .chain_err(|| "can not build a URL from the path")?;
        Ok(self.add_header(self.client.get(u.as_str())))
    }

    pub fn post<P: AsRef<str>>(&self, p: P) -> Result<RequestBuilder> {
        let u = self
            .server_url
            .join(p.as_ref())
            .chain_err(|| "can not build a URL from the path")?;
        Ok(self.add_header(self.client.post(u.as_str())))
    }
}
