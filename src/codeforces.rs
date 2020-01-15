use cookie_store::CookieStore;
use error_chain::bail;
use log::info;
use reqwest::header::{COOKIE, SET_COOKIE, USER_AGENT};
use reqwest::{ClientBuilder, RedirectPolicy, RequestBuilder, Response};
use std::io::{BufRead, Write};
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

pub struct CodeforcesBuilder {
    server_url: Url,
    identy: Option<String>,
    user_agent: String,
    cxx_dialect: String,
    py_dialect: String,
    retry_limit: i64,
    no_cookie: bool,
    cookie_file: Option<String>,
}

pub struct Codeforces {
    pub server_url: Url,
    pub identy: String,
    contest_url: Option<Url>,
    pub user_agent: String,
    pub prefer_cxx: String,
    pub prefer_py: String,
    pub retry_limit: i64,
    pub no_cookie: bool,
    pub cookie_file: Option<String>,
    cookie_store: CookieStore,
    client: reqwest::Client,
}

impl CodeforcesBuilder {
    pub fn build(self) -> Result<Codeforces> {
        if self.identy.is_none() {
            // bail!("identy is not set");
        }

        let cf = Codeforces {
            server_url: self.server_url,
            identy: self.identy.unwrap_or(String::from("")),
            contest_url: None,
            user_agent: self.user_agent,
            prefer_cxx: self.cxx_dialect,
            prefer_py: self.py_dialect,
            retry_limit: self.retry_limit,
            no_cookie: self.no_cookie,
            cookie_file: self.cookie_file,
            cookie_store: Default::default(),
            // We don't use redirection following feature of reqwest.
            // It will throw set-cookie in the header of redirect response.
            client: reqwest::Client::builder()
                .gzip(true)
                .redirect(RedirectPolicy::none())
                .build()
                .chain_err(|| "can not build HTTP client")?,
        };

        Ok(cf)
    }
}

impl Codeforces {
    pub fn builder() -> CodeforcesBuilder {
        let b = CodeforcesBuilder {
            server_url: Url::parse("https://codeforces.com").unwrap(),
            identy: None,
            user_agent: String::from(user_agent()),
            cxx_dialect: String::from("c++17"),
            py_dialect: String::from("py3"),
            retry_limit: 3,
            no_cookie: false,
            cookie_file: None,
        };

        b
    }

    pub fn new(b: ClientBuilder) -> Result<Self> {
        let cf = Codeforces {
            server_url: Url::parse("https://codeforces.com").unwrap(),
            identy: String::from(""),
            contest_url: None,
            user_agent: String::from(user_agent()),
            prefer_cxx: String::from("c++17"),
            prefer_py: String::from("py3"),
            retry_limit: 3,
            no_cookie: false,
            cookie_file: None,
            cookie_store: Default::default(),
            client: b.build().chain_err(|| "can not build HTTP client")?,
        };
        Ok(cf)
    }

    pub fn set_contest_path<S: ToString>(&mut self, s: S) -> Result<()> {
        let p = s.to_string() + "/";
        let u = self
            .server_url
            .join(&p)
            .chain_err(|| "can not build a legal URL from the contest path")?;
        self.contest_url = Some(u);
        Ok(())
    }

    pub fn get_contest_path(&self) -> Option<&str> {
        match &self.contest_url {
            Some(u) => Some(u.path()),
            None => None,
        }
    }

    pub fn get_contest_url(&self) -> Option<&Url> {
        self.contest_url.as_ref()
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
            Value::String(s) => self.set_contest_path(s)?,
            _ => (),
        }

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

        match &v["cookie_file"] {
            Value::String(s) => self.cookie_file = Some(s.to_string()),
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
        let cookie = self
            .cookie_store
            .iter_unexpired()
            .map(|c| c.encoded().to_string())
            .collect::<Vec<_>>()
            .join("; ");
        b.header(USER_AGENT, &self.user_agent)
            .header(COOKIE, &cookie)
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

    pub fn store_cookie(&mut self, resp: &Response) -> Result<()> {
        let u = Url::parse(resp.url().as_str()).chain_err(|| "bad url")?;
        resp.headers()
            .get_all(SET_COOKIE)
            .iter()
            .try_for_each(|val| -> Result<()> {
                let s = val.to_str().chain_err(|| "bad cookie string")?;
                self.cookie_store
                    .parse(s, &u)
                    .chain_err(|| "ill-formed cookie string")?;
                Ok(())
            })?;
        Ok(())
    }

    pub fn save_cookie<W: Write>(&self, w: &mut W) -> Result<()> {
        if let Err(e) = self.cookie_store.save_json(w) {
            bail!("can not save cookie: {}", e);
        }
        Ok(())
    }

    pub fn load_cookie<R: BufRead>(&mut self, rd: R) -> Result<()> {
        match CookieStore::load_json(rd) {
            Err(e) => bail!("can not load cookie: {}", e),
            Ok(c) => self.cookie_store = c,
        };
        Ok(())
    }

    pub fn judgement_protocol(&self, my: &Url, id: &str, csrf: &str) -> Result<String> {
        let u = self
            .server_url
            .join(my.path())
            .unwrap()
            .join("../../data/")
            .unwrap()
            .join("judgeProtocol")
            .unwrap();
        let mut params = std::collections::HashMap::new();
        params.insert("submissionId", id);
        params.insert("csrf_token", csrf);

        let post = self
            .post(u.as_str())
            .chain_err(|| "can not build XHR request")?
            .form(&params);

        let mut resp = post.send().chain_err(|| "can not send XHR request")?;
        resp.json().chain_err(|| "can not parse XHR response")
    }
}
