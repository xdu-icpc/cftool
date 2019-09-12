mod codeforces;
mod verdict;
use codeforces::Codeforces;
use log::{debug, error, info, warn};
use reqwest::{RedirectPolicy, Response};
use std::error::Error;
use std::process::exit;
use url::Url;

#[derive(Debug)]
struct CSRFError;

impl std::error::Error for CSRFError {}

impl std::fmt::Display for CSRFError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "no CSRF token found")
    }
}

fn get_csrf_token(resp: &mut Response) -> Result<String, Box<dyn Error>> {
    use regex::Regex;
    let re = Regex::new(r"meta name=.X-Csrf-Token. content=.(.*)./>").unwrap();
    let txt = resp.text()?;
    let cap = re.captures(&txt);
    let cap = match cap {
        Some(cap) => cap,
        None => return Err(Box::new(CSRFError {})),
    };
    let csrf = match cap.get(1) {
        Some(csrf) => csrf.as_str(),
        None => return Err(Box::new(CSRFError {})),
    };
    Ok(String::from(csrf))
}

fn http_get(url: &Url, cfg: &Codeforces) -> Response {
    info!("GET {} from {}", url.path(), url.host().unwrap());

    let resp = cfg
        .http_request_retry(|| cfg.get(url.path()).unwrap())
        .unwrap_or_else(|e| {
            error!("GET {} failed: {}", url.path(), e);
            exit(1);
        });

    if !resp.status().is_success() && !resp.status().is_redirection() {
        error!("GET {} failed with status: {}", url.path(), resp.status());
        exit(1);
    }

    resp
}

fn override_config(cfg: &mut Codeforces, p: &std::path::Path) {
    debug!("trying to read user config file {}", p.display());
    cfg.from_file(p).unwrap_or_else(|err| {
        error!("can not custom config file {}: {}", p.display(), err);
        exit(1);
    });
    info!("loaded custom config file {}", p.display());
}

fn get_lang(cfg: &Codeforces, ext: &str) -> &'static str {
    let lang_cxx = match cfg.prefer_cxx.as_str() {
        "c++17" => "54",
        "c++14" => "50",
        "c++11" => "42",
        _ => {
            error!("prefer_cxx must be one of c++17, c++14, or c++11");
            exit(1);
        }
    };

    let lang_py = match cfg.prefer_py.as_str() {
        "py3" => "31",
        "py2" => "7",
        "pypy3" => "41",
        "pypy2" => "40",
        _ => {
            error!("prefer_py must be one of py3, py2, pypy3, or pypy2");
            exit(1);
        }
    };

    match ext {
        "c" => "43",
        "cc" | "cp" | "cxx" | "cpp" | "CPP" | "c++" | "C" => lang_cxx,
        "py" => lang_py,
        "rs" => "49",
        "java" => "36",
        _ => {
            error!("don't know extension {}", ext);
            exit(1);
        }
    }
}

fn maybe_save_cookie(cf: &Codeforces, path: &std::path::Path) {
    debug!("try saving cookie to cache {}", path.display());

    let f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path);

    match f {
        Err(e) => {
            error!(
                "can not open cache file {} for writing: {}",
                path.display(),
                e
            );
            error!("cookie not saved");
            return;
        }
        Ok(mut f) => {
            if let Err(e) = cf.save_cookie(&mut f) {
                error!("can not write into cache file {}: {}", path.display(), e);
            } else {
                info!("cookie saved to cache {}", path.display());
            }
        }
    }
}

fn maybe_load_cookie(cf: &mut Codeforces, path: &std::path::Path) {
    debug!("try loading cookie from cache {}", path.display());

    if path.exists() {
        let f = std::fs::File::open(path).unwrap_or_else(|err| {
            error!(
                "can not open cache file {} for reading: {}",
                path.display(),
                err
            );
            exit(1);
        });
        use std::io::BufReader;
        let r = BufReader::new(f);
        cf.load_cookie(r).unwrap_or_else(|err| {
            error!("can not read cache file: {}", err);
            exit(1);
        });
    } else {
        info!("cookie cache {} does not exist", path.display());
    }
}

fn print_verdict(resp: &mut Response) -> bool {
    use termcolor::ColorChoice::Auto;
    use termcolor::StandardStream;
    use verdict::Verdict;
    let mut w = StandardStream::stdout(Auto);

    let v = Verdict::parse(resp).unwrap_or_else(|e| {
        error!("can not get verdict from response: {}", e);
        exit(1);
    });

    v.print(&mut w).unwrap_or_else(|e| {
        error!("can not print verdict: {}", e);
        exit(1);
    });

    match v {
        Verdict::Waiting(_) => true,
        _ => false,
    }
}

fn poll_or_query_verdict(url: &Url, cfg: &Codeforces, poll: bool) {
    use std::time::{Duration, SystemTime};
    let mut wait = true;
    while wait {
        let next_try = SystemTime::now() + Duration::new(5, 0);
        let mut resp = http_get(url, cfg);
        wait = print_verdict(&mut resp) && poll;
        if !wait {
            break;
        }
        if let Ok(d) = next_try.duration_since(SystemTime::now()) {
            std::thread::sleep(d);
        }
    }
}

enum Action {
    None,
    Dry,
    Query,
    Submit(String),
}

fn main() {
    use clap::{App, Arg};
    let matches = App::new("XDU-ICPC cftool")
        .version("0.1.1")
        .author("Xi Ruoyao <xry111@mengyan1223.wang>")
        .about("A command line tool for submitting code to Codeforces")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .takes_value(true)
                .value_name("FILE")
                .help(
                    "Sets a custom config file, \
                     overriding other config files",
                ),
        )
        .arg(
            Arg::with_name("problem")
                .short("p")
                .long("problem")
                .takes_value(true)
                .value_name("A-Z")
                .help("The problem ID in contest"),
        )
        .arg(
            Arg::with_name("source")
                .short("s")
                .long("source")
                .takes_value(true)
                .value_name("FILE")
                .help("The source code file to be submitted"),
        )
        .arg(
            Arg::with_name("v")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )
        .arg(
            Arg::with_name("dry-run")
                .long("dry-run")
                .short("d")
                .takes_value(false)
                .help("Only do authentication"),
        )
        .arg(
            Arg::with_name("query")
                .long("query")
                .short("q")
                .takes_value(false)
                .help("Query the status of the last submission"),
        )
        .arg(
            Arg::with_name("poll")
                .long("poll")
                .short("l")
                .takes_value(false)
                .help(
                    "Polling the last submission until it's judged,\
                     implies -q if -p is not used",
                ),
        )
        .arg(
            Arg::with_name("contest")
                .long("contest")
                .short("o")
                .value_name("PATH")
                .help("Contest path, overriding the config files")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("server")
                .value_name("scheme://domain")
                .long("server")
                .short("u")
                .help("Server URL, overriding the config files")
                .takes_value(true),
        )
        .get_matches();

    let v = matches.occurrences_of("v") as usize;
    stderrlog::new()
        .module(module_path!())
        .verbosity(v + 1)
        .init()
        .unwrap();

    info!("{}", "this is XDU-ICPC cftool, version 0.1.1");

    let mut action = Action::None;

    if let Some(problem) = matches.value_of("problem") {
        if problem.len() != 1 || !('A'..'Z').contains(&problem.chars().next().unwrap()) {
            error!("{} is impossible to be a problem ID", problem);
            exit(1);
        }
        action = Action::Submit(String::from(problem));
    }

    let conflict_msg = "can only use one of --dry-run, --query,\
                        and --problem";
    if matches.occurrences_of("dry-run") > 0 {
        if let Action::None = action {
            action = Action::Dry;
        } else {
            error!("{}", conflict_msg);
            exit(1);
        }
    }

    if matches.occurrences_of("query") > 0 {
        if let Action::None = action {
            action = Action::Query;
        } else {
            error!("{}", conflict_msg);
            exit(1);
        }
    }

    let need_poll = matches.occurrences_of("poll") > 0;
    if need_poll {
        if let Action::None = action {
            action = Action::Query;
        }
    }

    if let Action::None = action {
        error!("must use one of --dry-run, --query, and --problem");
        exit(1);
    }

    let source = matches.value_of("source").unwrap_or("");
    let ext = if let Action::Submit(_) = action {
        std::path::Path::new(source)
            .extension()
            .unwrap_or_else(|| {
                error!(
                    "no extension in filename {}, \
                     can not determine the language",
                    source
                );
                exit(1);
            })
            .to_str()
            .unwrap_or_else(|| {
                error!(
                    "extension of {} is not valid UTF-8, \
                     can not determine the language",
                    source
                );
                exit(1);
            })
    } else {
        ""
    };

    // We don't use redirection following feature of reqwest.
    // It will throw set-cookie the header of redirect response.
    let client_builder = reqwest::Client::builder()
        .gzip(true)
        .redirect(RedirectPolicy::none());

    let mut cfg = Codeforces::new(client_builder).unwrap();

    let project_dirs = directories::ProjectDirs::from("cn.edu.xidian.acm", "XDU-ICPC", "cftool");

    // Override configuration from user config file.
    match &project_dirs {
        Some(dir) => {
            let config_file = dir.config_dir().join("cftool.json");
            if config_file.exists() {
                override_config(&mut cfg, &config_file);
            } else {
                info!("user config file {} does not exist", config_file.display());
            }
            ()
        }
        None => {
            warn!("can not get the path of user config file on the system");
            ()
        }
    };

    // Override configuration from the config file in working directory.
    debug!(
        "trying to read config file cftool.json in the working \
         directory"
    );
    let config_file = std::path::Path::new("cftool.json");
    if config_file.exists() {
        override_config(&mut cfg, &config_file);
    } else {
        debug!("cftool.json does not exist")
    }

    let custom_config = matches.value_of("config").unwrap_or("");
    if custom_config != "" {
        let path = std::path::Path::new(custom_config);
        override_config(&mut cfg, &path);
    }

    let contest_override = matches.value_of("contest").unwrap_or("");
    if contest_override != "" {
        cfg.contest_path = String::from(contest_override);
    }

    let server_override = matches.value_of("server").unwrap_or("");
    if server_override != "" {
        cfg.server_url = Url::parse(server_override).unwrap_or_else(|e| {
            error!("can not parse url {}: {}", server_override, e);
            exit(1);
        });
    }

    match cfg.server_url.scheme() {
        "http" | "https" => (),
        _ => {
            error!("scheme {} is not implemented", cfg.server_url.scheme());
            exit(1);
        }
    };

    if cfg.server_url.host().is_none() {
        error!("host is empty");
        exit(1);
    }

    if cfg.identy == "" {
        error!("no identy provided");
        exit(1);
    }

    if project_dirs.is_none() {
        warn!(
            "do not know the user cache dir on this system, \
             cookie disabled"
        );
        cfg.no_cookie = true;
    }

    let cookie_file = if !cfg.no_cookie {
        let dir = project_dirs.unwrap();
        let cookie_dir = dir.cache_dir().join("cookie");
        std::fs::create_dir_all(&cookie_dir).unwrap_or_else(|err| {
            error!(
                "can not create cache dir {}: {}",
                cookie_dir.to_string_lossy(),
                err
            );
        });
        Some(cookie_dir.join(format!("{}.json", &cfg.identy)))
    } else {
        None
    };

    let lang = if let Action::Submit(_) = action {
        get_lang(&cfg, ext)
    } else {
        ""
    };

    if cfg.contest_path == "" {
        error!("no contest URL provided");
        exit(1);
    }

    cfg.contest_path += "/";

    let contest_url = cfg
        .server_url
        .join(&cfg.contest_path)
        .unwrap_or_else(|err| {
            error!("can not determine contest URL: {}", err);
            exit(1);
        });
    let submit_url = contest_url.join("submit").unwrap();

    match &cookie_file {
        Some(f) => maybe_load_cookie(&mut cfg, f.as_path()),
        _ => (),
    };

    let resp_try = http_get(&submit_url, &cfg);

    // The cookie contains session ID so we should save it.
    cfg.store_cookie(&resp_try).unwrap_or_else(|e| {
        error!("can not store cookie: {}", e);
        exit(1);
    });

    let mut resp = if resp_try.status().is_redirection() {
        // We are redirected.
        info!("authentication required");

        let login_url = cfg.server_url.join("enter").unwrap_or_else(|err| {
            error!("can not get login url: {}", err);
            exit(1);
        });

        let mut resp = http_get(&login_url, &cfg);
        let csrf = get_csrf_token(&mut resp).unwrap_or_else(|err| {
            error!("failed to get CSRF token: {}", err);
            exit(1);
        });

        debug!("CSRF token for /enter is {}", csrf);

        // Read password
        let prompt = format!("[cftool] password for {}: ", &cfg.identy);
        let passwd = rpassword::prompt_password_stderr(&prompt).unwrap_or_else(|err| {
            error!("failed reading password: {}", err);
            exit(1);
        });

        // Prepare the form data.
        use std::collections::HashMap;
        let mut params = HashMap::new();
        params.insert("handleOrEmail", cfg.identy.as_str());
        params.insert("password", passwd.as_str());
        params.insert("csrf_token", csrf.as_str());
        params.insert("bfaa", "");
        params.insert("ftaa", "");
        params.insert("action", "enter");
        params.insert("remember", "on");

        info!("POST /enter");
        let resp = cfg
            .post(login_url.as_str())
            .unwrap()
            .form(&params)
            .send()
            .unwrap_or_else(|err| {
                error!("POST /enter: {}", err);
                exit(1);
            });
        if !resp.status().is_success() && !resp.status().is_redirection() {
            error!("POST /enter: status = {}", resp.status());
            exit(1);
        }

        cfg.store_cookie(&resp).unwrap_or_else(|e| {
            error!("can not save cookie: {}", e);
            exit(1);
        });

        // Retry to GET the submit page.
        let resp = http_get(&submit_url, &cfg);
        if resp.status().is_redirection() {
            error!(
                "authentication failed, maybe identy or password is\
                 wrong"
            );
            exit(1);
        }
        resp
    } else {
        resp_try
    };

    match &cookie_file {
        Some(f) => maybe_save_cookie(&cfg, f),
        _ => (),
    };

    let problem = match action {
        Action::Submit(p) => p,
        Action::Dry => exit(0),
        Action::Query => {
            let my_url = contest_url.join("my").unwrap();
            poll_or_query_verdict(&my_url, &cfg, false);
            exit(0);
        }
        Action::None => unreachable!(),
    };

    let csrf = get_csrf_token(&mut resp).unwrap_or_else(|err| {
        error!("failed to get CSRF token: {}", err);
        exit(1);
    });

    debug!("CSRF token for {} is {}", submit_url.path(), csrf);

    use reqwest::multipart::{Form, Part};
    let src = Part::file(source).unwrap_or_else(|err| {
        error!("can not load file {} to be submitted: {}", source, err);
        exit(1);
    });
    let form = Form::new()
        .text("csrf_token", String::from(csrf))
        .text("ftaa", "")
        .text("bfaa", "")
        .text("action", "submitSolutionFormSubmitted")
        .text("submittedProblemIndex", problem)
        .text("programTypeId", lang)
        .text("source", "")
        .text("tabSize", "4")
        .part("sourceFile", src);

    info!("POST {}", submit_url.path());
    let resp = cfg
        .post(submit_url.as_str())
        .unwrap()
        .multipart(form)
        .send()
        .unwrap_or_else(|err| {
            error!("POST {} failed: {}", submit_url, err);
            exit(1);
        });

    if !resp.status().is_success() && !resp.status().is_redirection() {
        error!("POST {} failed with status: {}", submit_url, resp.status());
        exit(1);
    }

    if need_poll {
        let my_url = contest_url.join("my").unwrap();
        poll_or_query_verdict(&my_url, &cfg, true);
    }
}
