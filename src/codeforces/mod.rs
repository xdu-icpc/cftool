mod config;
use cookie_store::CookieStore;
use error_chain::bail;
use log::info;
use reqwest::header::{COOKIE, SET_COOKIE, USER_AGENT};
use reqwest::{RedirectPolicy, RequestBuilder, Response};
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

fn cxx_dialect_recognize(d: &str) -> Result<&'static str> {
    Ok(match d {
        "c++11" | "cxx11" | "cpp11" | "c++0x" | "cxx0x" | "cpp0x" => "c++11",
        "c++14" | "cxx14" | "cpp14" | "c++1y" | "cxx1y" | "cpp1y" => "c++14",
        "c++17" | "cxx17" | "cpp17" | "c++1z" | "cxx1z" | "cpp1z" => "c++17",
        _ => bail!("unknown or unsupported C++ dialect: {}", d),
    })
}

fn py_dialect_recognize(d: &str) -> Result<&'static str> {
    Ok(match d {
        "py2" | "python2" | "cpython2" => "py2",
        "py3" | "python3" | "cpython3" => "py3",
        "pypy2" => "pypy2",
        "pypy3" => "pypy3",
        _ => bail!("unknown or unsupported Python dialect: {}", d),
    })
}

enum CookieLocation {
    None,
    Dir(std::path::PathBuf),
    File(std::path::PathBuf),
}

struct CodeforcesBuilder {
    server_url: Url,
    identy: Option<String>,
    user_agent: String,
    cxx_dialect: &'static str,
    py_dialect: &'static str,
    cookie_location: CookieLocation,
    retry_limit: i64,
    no_cookie: bool,

    contest_url: Option<Url>,
}

pub struct CodeforcesBuilderResult {
    r: Result<CodeforcesBuilder>,
}

pub struct Codeforces {
    pub server_url: Url,
    pub identy: String,
    contest_url: Option<Url>,
    pub user_agent: String,
    pub cxx_dialect: &'static str,
    pub py_dialect: &'static str,
    pub retry_limit: i64,
    pub cookie_file: Option<std::path::PathBuf>,
    cookie_store: CookieStore,
    client: reqwest::Client,
}

impl CodeforcesBuilderResult {
    pub fn build(self) -> Result<Codeforces> {
        if let Err(e) = self.r {
            return Err(e);
        }

        let b = self.r.unwrap();

        if b.identy.is_none() {
            bail!("identy is not set");
        }

        let identy = b.identy.unwrap();

        let cookie_file = if b.no_cookie {
            None
        } else {
            match b.cookie_location {
                CookieLocation::None => None,
                CookieLocation::File(path) => Some(path),
                CookieLocation::Dir(dir) => Some(dir.join(format!("{}.json", identy))),
            }
        };

        match b.server_url.scheme() {
            "http" | "https" => (),
            _ => {
                bail!("scheme {} is not implemented", b.server_url.scheme());
            }
        };

        let mut cf = Codeforces {
            server_url: b.server_url,
            identy: identy,
            contest_url: b.contest_url,
            user_agent: b.user_agent,
            cxx_dialect: b.cxx_dialect,
            py_dialect: b.py_dialect,
            retry_limit: b.retry_limit,
            cookie_file: cookie_file,
            cookie_store: Default::default(),
            // We don't use redirection following feature of reqwest.
            // It will throw set-cookie in the header of redirect response.
            client: reqwest::Client::builder()
                .gzip(true)
                .redirect(RedirectPolicy::none())
                .build()
                .chain_err(|| "can not build HTTP client")?,
        };

        if let Err(e) = cf.load_cookie_from_file() {
            Err(e)
        } else {
            Ok(cf)
        }
    }

    fn is_err(&self) -> bool {
        return self.r.is_err();
    }

    fn from_err(e: Error) -> Self {
        Self { r: Err(e) }
    }

    pub fn server_url(self, u: Url) -> Self {
        if self.is_err() {
            return self;
        }

        let mut b = self.r.unwrap();
        b.server_url = u;
        Self { r: Ok(b) }
    }

    pub fn server_url_str(self, s: &str) -> Self {
        match Url::parse(s).chain_err(|| "can not parse url") {
            Err(e) => Self::from_err(e),
            Ok(u) => self.server_url(u),
        }
    }

    pub fn identy<S: ToString>(self, s: S) -> Self {
        if self.is_err() {
            return self;
        }
        let mut b = self.r.unwrap();
        b.identy = Some(s.to_string());
        Self { r: Ok(b) }
    }

    pub fn user_agent<S: ToString>(self, s: S) -> Self {
        if self.is_err() {
            return self;
        }
        let mut b = self.r.unwrap();
        b.user_agent = s.to_string();
        Self { r: Ok(b) }
    }

    pub fn cookie_file(self, path: std::path::PathBuf) -> Self {
        if self.is_err() {
            return self;
        }
        let mut b = self.r.unwrap();
        b.cookie_location = CookieLocation::File(path);
        Self { r: Ok(b) }
    }

    pub fn cookie_dir(self, path: std::path::PathBuf) -> Self {
        if self.is_err() {
            return self;
        }
        let mut b = self.r.unwrap();
        b.cookie_location = CookieLocation::Dir(path);
        Self { r: Ok(b) }
    }

    pub fn no_cookie(self, value: bool) -> Self {
        if self.is_err() {
            return self;
        }
        let mut b = self.r.unwrap();
        b.no_cookie = value;
        Self { r: Ok(b) }
    }

    pub fn retry_limit(self, value: i64) -> Self {
        if self.is_err() {
            return self;
        }
        let mut b = self.r.unwrap();
        b.retry_limit = value;
        Self { r: Ok(b) }
    }

    pub fn cxx_dialect<S: ToString>(self, s: S) -> Self {
        if self.is_err() {
            return self;
        }
        let s = s.to_string();
        let dialect = cxx_dialect_recognize(&s);
        if let Err(e) = dialect {
            return Self::from_err(e);
        }
        let mut b = self.r.unwrap();
        b.cxx_dialect = dialect.unwrap();
        Self { r: Ok(b) }
    }

    pub fn py_dialect<S: ToString>(self, s: S) -> Self {
        if self.is_err() {
            return self;
        }
        let s = s.to_string();
        let dialect = py_dialect_recognize(&s);
        if let Err(e) = dialect {
            return Self::from_err(e);
        }
        let mut b = self.r.unwrap();
        b.py_dialect = dialect.unwrap();
        Self { r: Ok(b) }
    }

    pub fn contest_path<S: ToString>(self, s: S) -> Self {
        if let Err(e) = self.r {
            return Self::from_err(e);
        }

        let mut b = self.r.unwrap();
        let p = s.to_string() + "/";
        let u = b
            .server_url
            .join(&p)
            .chain_err(|| "can not build a legal URL from the contest path");

        if let Err(e) = u {
            return Self::from_err(e);
        }

        b.contest_url = Some(u.unwrap());
        Self { r: Ok(b) }
    }

    // Override some config options from JSON config file.
    pub fn set_from_file<P: AsRef<Path>>(mut self, path: P) -> Self {
        use std::fs::File;
        use std::io::BufReader;
        let file = File::open(path).chain_err(|| "can not open file");
        if let Err(e) = file {
            return Self::from_err(e);
        }
        let rdr = BufReader::new(file.unwrap());

        let cfg: Result<config::Config> =
            serde_json::from_reader(rdr).chain_err(|| "can not parse json");
        if let Err(e) = cfg {
            return Self::from_err(e);
        }
        let cfg = cfg.unwrap();

        if let Some(s) = cfg.contest_path {
            self = self.contest_path(s);
        }

        if let Some(s) = cfg.server_url {
            self = self.server_url_str(&s);
        }

        if let Some(s) = cfg.identy {
            self = self.identy(s)
        }

        if let Some(s) = cfg.user_agent {
            self = self.user_agent(s)
        }

        if let Some(s) = cfg.prefer_cxx {
            self = self.cxx_dialect(s)
        }

        if let Some(s) = cfg.prefer_py {
            self = self.py_dialect(s)
        }

        if let Some(s) = cfg.cookie_file {
            self = self.cookie_file(s)
        }

        if let Some(x) = cfg.retry_limit {
            self = self.retry_limit(x);
        }

        if let Some(b) = cfg.no_cookie {
            self = self.no_cookie(b);
        }

        self
    }
}

impl Codeforces {
    pub fn builder() -> CodeforcesBuilderResult {
        let b = CodeforcesBuilder {
            server_url: Url::parse("https://codeforces.com").unwrap(),
            identy: None,
            user_agent: String::from(user_agent()),
            cxx_dialect: "c++17",
            py_dialect: "py3",
            retry_limit: 3,
            no_cookie: false,
            cookie_location: CookieLocation::None,
            contest_url: None,
        };

        CodeforcesBuilderResult { r: Ok(b) }
    }

    fn load_cookie_from_file(&mut self) -> Result<()> {
        if self.cookie_file == None {
            return Ok(());
        }

        let path = self.cookie_file.as_ref().unwrap();
        if path.exists() {
            let f = std::fs::File::open(path)
                .chain_err(|| format!("can not open cache file {} for reading", path.display()))?;
            use std::io::BufReader;
            let r = BufReader::new(f);
            self.load_cookie(r)
        } else {
            Ok(())
        }
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

    fn load_cookie<R: BufRead>(&mut self, rd: R) -> Result<()> {
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
