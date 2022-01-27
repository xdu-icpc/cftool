use cookie_store::CookieStore;
use error_chain::bail;
use reqwest::blocking::RequestBuilder;
use reqwest::header::USER_AGENT;
use reqwest::redirect;
use reqwest::Method;
use reqwest_cookie_store::CookieStoreMutex;
use std::io::Write;
use std::path::{Path, PathBuf};
use url::Url;

mod config;
mod language;
mod response;
mod verdict;

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

fn check_url_scheme(s: &str) -> Result<Url> {
    let u = Url::parse(s).chain_err(|| "can not parse URL")?;
    match u.scheme() {
        "https" => Ok(u),
        "http" => bail!("plain HTTP is insecure, use HTTPS instead"),
        _ => bail! {"unsupported protocol {}", u.scheme()},
    }
}

fn load_cookie_from_file(f: Option<&PathBuf>) -> Result<CookieStore> {
    let path = if let Some(value) = f {
        value
    } else {
        return Ok(Default::default());
    };

    if path.exists() {
        let f = std::fs::File::open(path)
            .chain_err(|| format!("can not open cache file {} for reading", path.display()))?;
        use std::io::BufReader;
        let r = BufReader::new(f);
        CookieStore::load_json(r).map_err(|e| format!("can not load cookie: {}", e).into())
    } else {
        Ok(Default::default())
    }
}

pub struct CodeforcesBuilder {
    server_url: Option<String>,
    identy: Option<String>,
    user_agent: Option<String>,
    cxx_dialect: Option<String>,
    py_dialect: Option<String>,
    rust_edition: Option<String>,
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

        let server_url = check_url_scheme(
            b.server_url
                .as_ref()
                .map_or("https://codeforces.com", |x| x.as_ref()),
        )?;

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
        let rs = b.rust_edition.as_ref().map_or("2018", |x| x.as_ref());

        let dialect = language::DialectParser::new(cxx, py, rs)
            .chain_err(|| "can not parse dialect setting")?;

        const VERSION: &str =
            git_version::git_version!(args = ["--tags", "--always", "--dirty=-modified"]);
        let user_agent = b
            .user_agent
            .unwrap_or(format!("cftool/{} (cftool)", VERSION));

        let cookie_store = load_cookie_from_file(cookie_file.as_ref())
            .map(CookieStoreMutex::new)
            .map(std::sync::Arc::new)?;

        let cf = Codeforces {
            server_url,
            identy,
            contest_url,
            user_agent,
            dialect,
            retry_limit: b.retry_limit,
            cookie_file,
            cookie_store: std::sync::Arc::clone(&cookie_store),
            // We don't use redirection following feature of reqwest.
            // It will throw set-cookie in the header of redirect response.
            client: reqwest::blocking::Client::builder()
                .redirect(redirect::Policy::none())
                .http2_prior_knowledge()
                .cookie_provider(std::sync::Arc::clone(&cookie_store))
                .build()
                .chain_err(|| "can not build HTTP client")?,
            csrf: None,
        };
        Ok(cf)
    }

    pub fn have_server_url_override(&self) -> bool {
        self.server_url.is_some()
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

    pub fn rust_edition<S: ToString>(mut self, s: S) -> Self {
        self.rust_edition = Some(s.to_string());
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

        if let Some(s) = cfg.rust_edition {
            self = self.rust_edition(s)
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
        get_csrf_token_str(txt)
    } else {
        None
    }
}

pub struct Codeforces {
    server_url: Url,
    identy: String,
    contest_url: Url,
    user_agent: String,
    dialect: language::DialectParser,
    retry_limit: i64,
    cookie_file: Option<PathBuf>,
    cookie_store: std::sync::Arc<CookieStoreMutex>,
    client: reqwest::blocking::Client,
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
            rust_edition: None,
            retry_limit: 3,
            no_cookie: false,
            cookie_location: CookieLocation::None,
            contest_path: None,
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

    fn http_get<P: AsRef<str>>(&mut self, path: P) -> Result<Response> {
        self.http_request(Method::GET, path, Ok, true)
    }

    fn http_request<P, F>(
        &mut self,
        method: Method,
        path: P,
        decorator: F,
        retry: bool,
    ) -> Result<Response>
    where
        P: AsRef<str>,
        F: Fn(RequestBuilder) -> Result<RequestBuilder>,
    {
        self.csrf = None;
        let mut retry_limit = if retry { self.retry_limit } else { 1 };
        let resp = loop {
            let method = method.clone();
            let u = self
                .server_url
                .join(path.as_ref())
                .chain_err(|| "can not build a URL from the path")?;
            let resp = decorator(self.add_header(self.client.request(method, u.as_str())))?.send();

            if let Err(e) = &resp {
                if e.is_timeout() && retry_limit > 0 {
                    retry_limit -= 1;
                    continue;
                }
            }
            break resp;
        };

        let resp = resp
            .chain_err(|| "can not send HTTP request")?
            .try_into()
            .chain_err(|| "bad HTTP response")?;

        self.csrf = get_csrf_token(&resp);
        Ok(resp)
    }

    fn add_header(&self, b: RequestBuilder) -> RequestBuilder {
        b.header(USER_AGENT, &self.user_agent)
    }

    fn save_cookie<W: Write>(&self, w: &mut W) -> Result<()> {
        let store = match self.cookie_store.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        store
            .save_json(w)
            .map_err(|e| format!("cannot save cookie: {}", e).into())
    }

    pub fn judgement_protocol(&mut self, id: &str) -> Result<String> {
        let csrf = self.get_csrf_token()?;
        // XHR can reuse csrf token
        self.csrf = Some(csrf.clone());

        let u = self
            .contest_url
            .join("../../data/judgeProtocol")
            .chain_err(|| "cannot make judgement protocol URL")?;
        let mut params = std::collections::HashMap::new();
        params.insert("submissionId", id);
        params.insert("csrf_token", &csrf);

        let resp = self.http_request(Method::POST, u.as_str(), |x| Ok(x.form(&params)), true)?;
        if let Response::Content(data) = resp {
            Ok(serde_json::from_str(&data).chain_err(|| "cannot parse JSON")?)
        } else {
            bail!("response {:?} has no content", resp);
        }
    }

    pub fn probe_login_status(&mut self) -> Result<bool> {
        let submit_url = self
            .server_url
            .join("/usertalk")
            .chain_err(|| "can not parse URL for probing login status")?;
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

        let csrf = self.get_csrf_token()?;

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
            .http_request(Method::POST, login_url, |x| Ok(x.form(&params)), false)
            .chain_err(|| "POST /enter")?;

        if let Response::Other(status) = resp {
            bail!("POST /enter: status = {}", status);
        }

        Ok(())
    }

    fn get_csrf_token(&mut self) -> Result<String> {
        let csrf = self.csrf.take();
        if let Some(value) = csrf {
            return Ok(value);
        }
        self.http_get(self.server_url.clone())?;
        self.csrf.take().chain_err(|| "can not get CSRF token")
    }

    pub fn get_last_submission(&mut self) -> Result<String> {
        let url = self
            .contest_url
            .join("my?cftool=1")
            .chain_err(|| "cannot generate status URL")?;
        let resp = self.http_get(url).chain_err(|| "cannot GET status page")?;
        let txt = if let Response::Content(t) = resp {
            t
        } else {
            bail!("response {:?} has no content", resp);
        };
        verdict::parse_submission_id(&txt).chain_err(|| "cannot parse verdict")
    }

    pub fn get_verdict(&mut self, id: &str) -> Result<Verdict> {
        let csrf = self.get_csrf_token()?;
        // XHR can reuse csrf token
        self.csrf = Some(csrf.clone());

        let u = self
            .contest_url
            .join("../../data/submissionVerdict")
            .chain_err(|| "cannot make verdict data URL")?;
        let mut params = std::collections::HashMap::new();
        params.insert("submissionId", id);
        params.insert("csrf_token", &csrf);
        let resp = self.http_request(Method::POST, u.as_str(), |x| Ok(x.form(&params)), true)?;

        let txt = if let Response::Content(c) = &resp {
            c
        } else {
            bail!("response {} have no content");
        };

        Verdict::from_json(txt).chain_err(|| "can not parse verdict")
    }

    pub fn get_identy(&self) -> &str {
        self.identy.as_str()
    }

    pub fn submit(&mut self, problem: &str, src_path: &str, dialect: Option<&str>) -> Result<()> {
        let dialect = match dialect {
            Some(d) => language::get_lang_dialect(d),
            None => {
                let ext = std::path::Path::new(src_path)
                    .extension()
                    .chain_err(|| "source file has no extension")?
                    .to_str()
                    .chain_err(|| "source file extension is not UTF-8")?;
                self.dialect.get_lang_ext(ext)
            }
        }
        .chain_err(|| "cannot determine source file language")?;

        let url = self
            .contest_url
            .join("submit")
            .chain_err(|| "cannot build submit URL")?;

        let csrf = self.get_csrf_token()?;

        let resp = self.http_request(
            Method::POST,
            &url,
            |x| {
                use reqwest::blocking::multipart::{Form, Part};
                let src = Part::file(src_path).chain_err(|| format!("cannot load {}", src_path))?;

                let form = Form::new()
                    .text("csrf_token", csrf.clone())
                    .text("action", "submitSolutionFormSubmitted")
                    .text("submittedProblemIndex", problem.to_owned())
                    .text("programTypeId", dialect)
                    .text("tabSize", "4")
                    .text("sourceCodeConfirmed", "true")
                    .part("sourceFile", src);
                Ok(x.multipart(form))
            },
            false,
        )?;

        match resp {
            Response::Other(status) => bail!("POST failed, status = {}", status),
            Response::Content(_) => bail!(
                "server don't like the code, recheck \
                - maybe submitting same code multiple times?"
            ),
            Response::Redirection(u) => {
                if u != self.contest_url.join("my").unwrap() {
                    bail!(
                        "server don't like the code, recheck \
                        - maybe submitting to a nonexist problem?"
                    );
                }
                Ok(())
            }
        }
    }
}
