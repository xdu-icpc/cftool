use cookie_store::CookieStore;
use error_chain::bail;
use log::info;
use reqwest::header::{COOKIE, SET_COOKIE, USER_AGENT};
use reqwest::{Method, RedirectPolicy, RequestBuilder};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use url::Url;

mod config;
pub mod language;
pub mod response;
pub mod verdict;

pub type Response = response::Response;
pub type Verdict = verdict::Verdict;

mod error {
    error_chain::error_chain! {}
}

use error::*;

enum CookieLocation {
    None,
    Dir(PathBuf),
    File(PathBuf),
}

pub struct CodeforcesBuilder {
    server_url: Option<String>,
    identy: Option<String>,
    user_agent: Option<String>,
    cxx_dialect: Option<String>,
    py_dialect: Option<String>,
    cookie_location: CookieLocation,
    retry_limit: i64,
    no_cookie: bool,

    contest_path: Option<String>,
}

impl CodeforcesBuilder {
    pub fn build(self) -> Result<Codeforces> {
        let b = self;

        let identy = if let Some(value) = b.identy {
            value
        } else {
            bail!("identy is not set");
        };

        let cookie_file = if b.no_cookie {
            None
        } else {
            match b.cookie_location {
                CookieLocation::None => None,
                CookieLocation::File(path) => Some(path),
                CookieLocation::Dir(dir) => Some(dir.join(format!("{}.json", identy))),
            }
        };

        let server_url = Url::parse(
            b.server_url
                .as_ref()
                .map_or("https://codeforces.com", |x| x.as_ref()),
        )
        .chain_err(|| "can not parse server URL")?;

        match server_url.scheme() {
            "http" | "https" => (),
            _ => {
                bail!("scheme {} is not implemented", server_url.scheme());
            }
        };

        let contest_path = if let Some(value) = b.contest_path {
            value
        } else {
            bail!("contest path is not set");
        };

        let contest_url = server_url
            .join(&contest_path)
            .chain_err(|| "can not parse contest path into URL")?;

        let cxx = b.cxx_dialect.as_ref().map_or("c++17-64", |x| x.as_ref());
        let py = b.py_dialect.as_ref().map_or("py3", |x| x.as_ref());

        let dialect =
            language::DialectParser::new(cxx, py).chain_err(|| "can not parse dialect setting")?;

        let user_agent = b.user_agent.unwrap_or("cftool/0.4.1 (cftool)".to_owned());

        let mut cf = Codeforces {
            server_url: server_url,
            identy: identy,
            contest_url: contest_url,
            user_agent: user_agent,
            dialect: dialect,
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
            csrf: None,
        };

        if let Err(e) = cf.load_cookie_from_file() {
            Err(e)
        } else {
            Ok(cf)
        }
    }

    pub fn server_url(mut self, u: &str) -> Self {
        self.server_url = Some(u.to_owned());
        self
    }

    pub fn identy<S: ToString>(mut self, s: S) -> Self {
        self.identy = Some(s.to_string());
        self
    }

    pub fn user_agent<S: ToString>(mut self, s: S) -> Self {
        self.user_agent = Some(s.to_string());
        self
    }

    pub fn cookie_file(mut self, path: PathBuf) -> Self {
        self.cookie_location = CookieLocation::File(path);
        self
    }

    pub fn cookie_dir(mut self, path: PathBuf) -> Self {
        self.cookie_location = CookieLocation::Dir(path);
        self
    }

    pub fn no_cookie(mut self, value: bool) -> Self {
        self.no_cookie = value;
        self
    }

    pub fn retry_limit(mut self, value: i64) -> Self {
        self.retry_limit = value;
        self
    }

    pub fn cxx_dialect<S: ToString>(mut self, s: S) -> Self {
        self.cxx_dialect = Some(s.to_string());
        self
    }

    pub fn py_dialect<S: ToString>(mut self, s: S) -> Self {
        self.py_dialect = Some(s.to_string());
        self
    }

    pub fn contest_path<S: ToString>(mut self, s: S) -> Self {
        /* '/' for url::Url::join interface. */
        self.contest_path = Some(s.to_string() + "/");
        self
    }

    // Override some config options from JSON config file.
    pub fn set_from_file<P: AsRef<Path>>(mut self, path: P) -> Result<Self> {
        use std::fs::File;
        use std::io::BufReader;
        let file = File::open(path).chain_err(|| "can not open file")?;
        let rdr = BufReader::new(file);

        let cfg: config::Config =
            serde_json::from_reader(rdr).chain_err(|| "can not parse json")?;

        if let Some(s) = cfg.contest_path {
            self = self.contest_path(s);
        }

        if let Some(s) = cfg.server_url {
            self = self.server_url(&s);
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

        Ok(self)
    }
}

fn get_csrf_token_str(txt: &str) -> Option<String> {
    use regex::Regex;
    let re = Regex::new(r"meta name=.X-Csrf-Token. content=.(.*)./>").unwrap();
    let cap = re.captures(txt);
    let cap = match cap {
        Some(cap) => cap,
        None => return None,
    };
    let csrf = match cap.get(1) {
        Some(csrf) => csrf.as_str(),
        None => return None,
    };
    Some(String::from(csrf))
}

fn get_csrf_token(resp: &Response) -> Option<String> {
    if let Response::Content(txt) = resp {
        get_csrf_token_str(&txt)
    } else {
        None
    }
}

pub struct Codeforces {
    pub server_url: Url,
    pub identy: String,
    contest_url: Url,
    user_agent: String,
    pub dialect: language::DialectParser,
    retry_limit: i64,
    cookie_file: Option<PathBuf>,
    cookie_store: CookieStore,
    client: reqwest::Client,
    csrf: Option<String>,
}

impl Codeforces {
    pub fn builder() -> CodeforcesBuilder {
        CodeforcesBuilder {
            server_url: None,
            identy: None,
            user_agent: None,
            cxx_dialect: None,
            py_dialect: None,
            retry_limit: 3,
            no_cookie: false,
            cookie_location: CookieLocation::None,
            contest_path: None,
        }
    }

    fn load_cookie_from_file(&mut self) -> Result<()> {
        let path = if let Some(value) = self.cookie_file.as_ref() {
            value
        } else {
            return Ok(());
        };

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

    pub fn maybe_save_cookie(&self) -> Result<Option<&PathBuf>> {
        let path = if let Some(value) = self.cookie_file.as_ref() {
            value
        } else {
            return Ok(None);
        };

        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .chain_err(|| "can not open cache file for writing")?;

        self.save_cookie(&mut f)?;
        Ok(self.cookie_file.as_ref())
    }

    pub fn get_contest_url(&self) -> &Url {
        &self.contest_url
    }

    fn is_ssl_redirection(&self, resp: &Response) -> bool {
        let url = if let Response::Redirection(url) = resp {
            url
        } else {
            return false;
        };

        url.scheme() == "https"
            && self.server_url.scheme() != "https"
            && self.server_url.host() == url.host()
    }

    fn ensure_ssl(&mut self) {
        self.server_url.set_scheme("https").unwrap();
        self.contest_url.set_scheme("https").unwrap();
    }

    pub fn http_get<P: AsRef<str>>(&mut self, path: P) -> Result<Response> {
        self.http_request(Method::GET, path, |x| x, true)
    }

    pub fn http_request<P, F>(
        &mut self,
        method: Method,
        path: P,
        decorator: F,
        retry: bool,
    ) -> Result<Response>
    where
        P: AsRef<str>,
        F: Fn(RequestBuilder) -> RequestBuilder,
    {
        self.csrf = None;
        let mut retry_limit = if retry { self.retry_limit } else { 1 };
        loop {
            let method = method.clone();
            let u = self
                .server_url
                .join(path.as_ref())
                .chain_err(|| "can not build a URL from the path")?;
            let resp = decorator(self.add_header(self.client.request(method, u.as_str()))).send();

            if let Err(e) = &resp {
                if e.is_timeout() && retry_limit > 0 {
                    retry_limit -= 1;
                    info!("timeout, retrying");
                    continue;
                }
            }

            let resp = resp.chain_err(|| "can not get response")?;

            self.store_cookie(&resp)
                .chain_err(|| "can not store cookie")?;

            let resp = Response::wrap(resp).chain_err(|| "glitched HTTP response");

            if let Ok(r) = &resp {
                if self.is_ssl_redirection(r) {
                    self.ensure_ssl();
                    continue;
                }
                self.csrf = get_csrf_token(r);
            }

            return resp.chain_err(|| "http request failed");
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

    pub fn store_cookie(&mut self, resp: &reqwest::Response) -> Result<()> {
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

    fn save_cookie<W: Write>(&self, w: &mut W) -> Result<()> {
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

    pub fn judgement_protocol(&mut self, id: &str, csrf: &str) -> Result<String> {
        let u = self
            .contest_url
            .join("../../data/judgeProtocol")
            .chain_err(|| "cannot make judgement protocol URL")?;
        let mut params = std::collections::HashMap::new();
        params.insert("submissionId", id);
        params.insert("csrf_token", csrf);

        let resp = self.http_request(Method::POST, u.as_str(), |x| x.form(&params), true)?;
        if let Response::Content(data) = resp {
            Ok(serde_json::from_str(&data).chain_err(|| "cannot parse JSON")?)
        } else {
            bail!("response have no content");
        }
    }

    pub fn probe_login_status(&mut self) -> Result<bool> {
        let submit_url = self
            .contest_url
            .join("submit")
            .chain_err(|| "can not parse URL for submitting")?;
        let resp = self
            .http_get(&submit_url)
            .chain_err(|| format!("GET {} failed", submit_url))?;

        match resp {
            Response::Redirection(_) => Ok(false),
            Response::Content(_) => Ok(true),
            Response::Other(status) => bail!("GET {}: status = {}", submit_url, status),
        }
    }

    pub fn login(&mut self, password: &str) -> Result<()> {
        let login_url = self
            .server_url
            .join("enter")
            .chain_err(|| "can not get login url: {}")?;

        let mut csrf = self.get_csrf_token();
        let server_url = self.server_url.clone();
        if csrf.is_none() {
            self.http_get(&server_url)?;
            csrf = self.get_csrf_token();
        }

        let csrf = csrf.chain_err(|| "failed to get CSRF token")?;

        // Prepare the form data.
        use std::collections::HashMap;
        let mut params = HashMap::new();
        let identy = self.identy.clone();
        params.insert("handleOrEmail", identy.as_str());
        params.insert("password", password);
        params.insert("csrf_token", csrf.as_str());
        params.insert("action", "enter");
        params.insert("remember", "on");

        let resp = self
            .http_request(Method::POST, login_url, |x| x.form(&params), false)
            .chain_err(|| "POST /enter")?;

        if let Response::Other(status) = resp {
            bail!("POST /enter: status = {}", status);
        }

        Ok(())
    }

    pub fn get_csrf_token(&mut self) -> Option<String> {
        self.csrf.take()
    }

    pub fn get_verdict(&mut self) -> Result<Verdict> {
        let url = self
            .contest_url
            .join("my")
            .chain_err(|| "cannot generate status URL")?;
        let resp = self.http_get(url).chain_err(|| "cannot GET status page")?;
        let txt = if let Response::Content(t) = resp {
            t
        } else {
            bail!("response does not have content");
        };
        Verdict::parse(&txt).chain_err(|| "cannot parse verdict")
    }
}
