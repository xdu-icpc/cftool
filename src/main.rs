mod codeforces;
mod verdict;
use codeforces::Codeforces;
use log::{debug, error, info, warn};
use reqwest::Response;
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

fn get_csrf_token_str(txt: &str) -> Result<String, CSRFError> {
    use regex::Regex;
    let re = Regex::new(r"meta name=.X-Csrf-Token. content=.(.*)./>").unwrap();
    let cap = re.captures(txt);
    let cap = match cap {
        Some(cap) => cap,
        None => return Err(CSRFError {}),
    };
    let csrf = match cap.get(1) {
        Some(csrf) => csrf.as_str(),
        None => return Err(CSRFError {}),
    };
    Ok(String::from(csrf))
}

fn get_csrf_token(resp: &mut Response) -> Result<String, Box<dyn Error>> {
    let txt = resp.text()?;
    Ok(get_csrf_token_str(&txt)?)
}

fn http_get(url: &Url, cfg: &mut Codeforces) -> Response {
    info!("GET {} from {}", url.path(), url.host().unwrap());

    let resp = cfg.http_get(url.path()).unwrap_or_else(|e| {
        error!("GET {} failed: {}", url.path(), e);
        exit(1);
    });

    if !resp.status().is_success() && !resp.status().is_redirection() {
        error!("GET {} failed with status: {}", url.path(), resp.status());
        exit(1);
    }

    resp
}

fn get_lang_dialect(dialect: &str) -> &'static str {
    match dialect {
        "c" => "43",
        "c++17" => "54",
        "c++14" => "50",
        "c++11" => "42",
        "py3" => "31",
        "py2" => "7",
        "pypy3" => "41",
        "pypy2" => "40",
        "rust" => "49",
        "java" => "36",
        _ => {
            error!("don't know dialect {}", dialect);
            exit(1);
        }
    }
}

fn get_lang_ext(cfg: &Codeforces, ext: &str) -> &'static str {
    let dialect = match ext {
        "c" => "c",
        "cc" | "cp" | "cxx" | "cpp" | "CPP" | "c++" | "C" => cfg.cxx_dialect,
        "py" => cfg.py_dialect,
        "rs" => "rust",
        "java" => "java",
        _ => {
            error!("don't know extension {}", ext);
            exit(1);
        }
    };
    get_lang_dialect(dialect)
}

fn set_from_file(
    b: codeforces::CodeforcesBuilder,
    p: &std::path::Path,
) -> codeforces::CodeforcesBuilder {
    match b.set_from_file(p) {
        Ok(b) => b,
        Err(e) => {
            error!("can not parse {}: {}", p.display(), e);
            exit(1);
        }
    }
}

fn maybe_save_cookie(cf: &Codeforces) {
    if cf.cookie_file == None {
        return;
    }

    let path = cf.cookie_file.as_ref().unwrap();
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

fn print_verdict(resp_text: &str, color: bool) -> verdict::Verdict {
    use termcolor::ColorChoice::Auto;
    use termcolor::{Buffer, BufferWriter};
    use verdict::Verdict;
    let w = BufferWriter::stdout(Auto);
    let mut buf = if color {
        w.buffer()
    } else {
        Buffer::no_color()
    };

    let v = Verdict::parse(resp_text).unwrap_or_else(|e| {
        error!("can not get verdict from response: {}", e);
        exit(1);
    });

    v.print(&mut buf).unwrap_or_else(|e| {
        error!("can not buffer verdict: {}", e);
        exit(1);
    });

    w.print(&buf).unwrap_or_else(|e| {
        error!("can not output verdict: {}", e);
        exit(1);
    });

    v
}

fn get_ce_info(cf: &mut Codeforces, id: &str, csrf: &str) -> String {
    cf.judgement_protocol(id, csrf).unwrap_or_else(|e| {
        error!("can not get compilation error info: {}", e);
        String::new()
    })
}

fn poll_or_query_verdict(url: &Url, cfg: &mut Codeforces, poll: bool, no_color: bool) {
    use std::time::{Duration, SystemTime};
    let mut wait = true;
    while wait {
        let next_try = SystemTime::now() + Duration::new(5, 0);
        let mut resp = http_get(url, cfg);
        let txt = resp.text().unwrap_or_else(|e| {
            error!("can not parse response body into text: {}", e);
            exit(1);
        });
        let v = print_verdict(&txt, !no_color);
        wait = v.is_waiting() && poll;

        if v.is_compilation_error() {
            let csrf = get_csrf_token_str(&txt);
            if let Err(e) = csrf {
                error!("can not get csrf token: {}", e);
                error!("skip compilation error info");
                return;
            }

            let s = get_ce_info(cfg, v.get_id(), &csrf.unwrap());
            println!("{}", "===================================");
            print!("{}", s);
        }

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
        .version("0.4.1")
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
                    "Polling the last submission until it's judged, \
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
        .arg(
            Arg::with_name("identy")
                .value_name("IDENTY")
                .long("identy")
                .short("i")
                .help("Identy, handle or email, overriding the config files")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("dialect")
                .value_name("DIALECT")
                .long("dialect")
                .short("a")
                .help("Language dialect, overriding config and filename")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("cookie")
                .value_name("FILE")
                .long("cookie")
                .short("k")
                .help("Cookie cache file, overriding the default")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("no-color")
                .takes_value(false)
                .long("no-color")
                .short("w")
                .help("Do not use color for verdict"),
        )
        .get_matches();

    let v = matches.occurrences_of("v") as usize;
    stderrlog::new()
        .module(module_path!())
        .verbosity(v + 1)
        .init()
        .unwrap();

    info!("{}", "this is XDU-ICPC cftool, version 0.4.1");

    let mut action = Action::None;

    if let Some(problem) = matches.value_of("problem") {
        if problem.len() != 1 || !('A'..'Z').contains(&problem.chars().next().unwrap()) {
            error!("{} is impossible to be a problem ID", problem);
            exit(1);
        }
        action = Action::Submit(String::from(problem));
    }

    let conflict_msg = "can only use one of --dry-run, --query, \
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
        match std::path::Path::new(source).extension() {
            Some(e) => e.to_str().unwrap_or_else(|| {
                error!(
                    "extension of {} is not valid UTF-8, \
                     can not determine the language",
                    source
                );
                exit(1);
            }),
            None => "",
        }
    } else {
        ""
    };

    let no_color = matches.occurrences_of("no-color") > 0;

    let mut builder = Codeforces::builder();
    let mut cookie_dir = None;

    let project_dirs = directories::ProjectDirs::from("cn.edu.xidian.acm", "XDU-ICPC", "cftool");
    match &project_dirs {
        Some(dir) => {
            // Override configuration from user config file.
            let config_file = dir.config_dir().join("cftool.json");
            if config_file.exists() {
                builder = set_from_file(builder, &config_file);
            } else {
                info!("user config file {} does not exist", config_file.display());
            }
            cookie_dir = Some(dir.cache_dir().join("cookie"));
            ()
        }
        None => {
            warn!(
                "can not get the path of user config file and cache file \
                 on the system, cookie won't be saved unless you specify the \
                 location"
            );
            ()
        }
    };

    let mut mkdir_fail = false;
    if let Some(d) = &cookie_dir {
        std::fs::create_dir_all(d).unwrap_or_else(|err| {
            error!("can not create cache dir {}: {}", d.display(), err);
            mkdir_fail = true;
        });
    }
    if mkdir_fail {
        cookie_dir = None;
    }

    // set up the default cache dir now so it may be overrided by config
    if let Some(dir) = cookie_dir {
        builder = builder.cookie_dir(dir);
    }

    // Override configuration from the config file in working directory.
    debug!(
        "trying to read config file cftool.json in the working \
         directory"
    );
    let config_file = std::path::Path::new("cftool.json");
    if config_file.exists() {
        builder = set_from_file(builder, &config_file);
    } else {
        debug!("cftool.json does not exist")
    }

    let custom_config = matches.value_of("config").unwrap_or("");
    if custom_config != "" {
        let path = std::path::Path::new(custom_config);
        builder = set_from_file(builder, &path);
    }

    if let Some(path) = matches.value_of("cookie") {
        builder = builder.cookie_file(std::path::PathBuf::from(path));
    }

    if let Some(server) = matches.value_of("server") {
        builder = builder.server_url(server);
    }

    if let Some(identy) = matches.value_of("identy") {
        builder = builder.identy(identy);
    }

    if let Some(contest) = matches.value_of("contest") {
        builder = builder.contest_path(contest);
    }

    let mut cfg = builder.build().unwrap_or_else(|e| {
        error!("can not build Codeforces client: {}", e);
        exit(1);
    });

    let lang = if let Action::Submit(_) = action {
        if let Some(d) = matches.value_of("dialect") {
            get_lang_dialect(d)
        } else {
            get_lang_ext(&cfg, ext)
        }
    } else {
        ""
    };

    let submit_url = cfg.get_contest_url().join("submit").unwrap();

    let resp_try = http_get(&submit_url, &mut cfg);

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

        let mut resp = http_get(&login_url, &mut cfg);
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
        let resp = http_get(&submit_url, &mut cfg);
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

    maybe_save_cookie(&cfg);

    let problem = match action {
        Action::Submit(p) => p,
        Action::Dry => exit(0),
        Action::Query => {
            let my_url = cfg.get_contest_url().join("my").unwrap();
            poll_or_query_verdict(&my_url, &mut cfg, false, no_color);
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
        .text("sourceCodeConfirmed", "true")
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

    if !resp.status().is_redirection() {
        if resp.status().is_success() {
            error!("Codeforces doesn't like the code, please recheck");
            exit(1);
        }
        error!("POST {} failed with status: {}", submit_url, resp.status());
        exit(1);
    }

    if need_poll {
        let my_url = cfg.get_contest_url().join("my").unwrap();
        poll_or_query_verdict(&my_url, &mut cfg, true, no_color);
    }
}
