mod app;
mod codeforces;
use codeforces::Codeforces;
use codeforces::Verdict;
use log::{debug, error, info, warn};
use std::process::exit;

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

fn print_verdict(v: &Verdict, color: bool, id: &str) {
    use std::io::Write;
    use termcolor::ColorChoice::Auto;
    use termcolor::{Buffer, BufferWriter};
    let w = BufferWriter::stdout(Auto);
    let mut buf = if color {
        w.buffer()
    } else {
        Buffer::no_color()
    };

    write!(&mut buf, "{} ", id).unwrap_or_else(|e| {
        error!("can not buffer submission ID: {}", e);
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
}

fn get_ce_info(cf: &mut Codeforces, id: &str) -> String {
    cf.judgement_protocol(id).unwrap_or_else(|e| {
        error!("can not get compilation error info: {}", e);
        String::new()
    })
}

fn poll_or_query_verdict(cf: &mut Codeforces, poll: bool, no_color: bool) {
    use std::time::{Duration, SystemTime};
    let mut wait = true;
    let id = cf.get_last_submission().unwrap_or_else(|e| {
        error!("cannot get ID of last submission: {}", e);
        exit(1);
    });

    info!("submission id = {}:", &id);

    while wait {
        let next_try = SystemTime::now() + Duration::new(5, 0);
        let v = cf.get_verdict(&id).unwrap_or_else(|e| {
            error!("cannot get verdict: {}", e);
            exit(1);
        });

        print_verdict(&v, !no_color, &id);
        wait = v.is_waiting() && poll;

        if v.is_compilation_error() {
            let s = get_ce_info(cf, &id);
            println!("===================================");
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
    Err(String),
}

impl Action {
    fn submit<T: ToString>(s: T, force: bool) -> Self {
        let s = s.to_string().to_uppercase();
        if !force {
            let re = regex::Regex::new(r"^[A-Z]([1-9][0-9]*)?$").unwrap();
            if !re.is_match(&s) {
                return Self::Err(format!("{} does not look like a problem ID", s));
            }
        }
        Self::Submit(s)
    }

    fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

fn main() {
    use app::Parser;
    let args = app::App::parse();
    let v = args.verbose.checked_add(1).unwrap_or(usize::MAX);
    let modules = &[module_path!(), "reqwest"];
    stderrlog::new()
        .modules(modules.iter().cloned())
        .verbosity(v)
        .init()
        .unwrap();

    info!("this is XDU-ICPC cftool, {}", app::VERSION);

    let mut action = Action::None;

    if let Some(problem) = args.problem {
        action = Action::submit(problem, args.force);
    }

    let conflict_msg = "can only use one of --dry-run, --query, \
                        and --problem";
    if args.dry_run {
        if let Action::None = action {
            action = Action::Dry;
        } else {
            error!("{}", conflict_msg);
            exit(1);
        }
    }

    if args.query {
        if let Action::None = action {
            action = Action::Query;
        } else {
            error!("{}", conflict_msg);
            exit(1);
        }
    }

    let need_poll = args.poll;

    if let Some(source) = args.source.as_ref() {
        match &action {
            Action::Dry | Action::Query => {
                error!(
                    "specifying source code file does not make sense \
                    without submitting it"
                );
                exit(1);
            }
            Action::Submit(_) => (),
            Action::None => {
                let path = std::path::Path::new(&source);
                if let Some(s) = path.file_stem().and_then(|x| x.to_str()) {
                    action = Action::submit(s, args.force);
                } else {
                    error!(
                        "can't guess problem ID from the filename, \
                        please specify it explicitly"
                    );
                }
                if let Action::Submit(problem) = &action {
                    info!("guessed problem ID to be {}", problem);
                }
            }
            Action::Err(_) => (),
        }
    }

    if need_poll && action.is_none() {
        action = Action::Query;
    }

    match &action {
        Action::None => {
            error!("must use one of --dry-run, --query, and --problem");
            exit(1);
        }
        Action::Submit(_) => {
            if args.source.is_none() {
                error!("attempt to submit, but no source code specified");
                exit(1);
            }
        }
        Action::Err(s) => {
            error!("{}", s);
            exit(1);
        }
        Action::Dry | Action::Query => (),
    };

    let no_color = args.no_color;

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
        }
        None => {
            warn!(
                "can not get the path of user config file and cache file \
                 on the system, cookie won't be saved unless you specify the \
                 location"
            );
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
        builder = set_from_file(builder, config_file);
    } else {
        debug!("cftool.json does not exist")
    }

    if let Some(custom_config) = args.config {
        let path = std::path::Path::new(&custom_config);
        builder = set_from_file(builder, path);
    }

    if let Some(path) = args.cookie {
        builder = builder.cookie_file(std::path::PathBuf::from(path));
    }

    if let Some(server) = args.server {
        builder = builder.server_url(&server);
    }

    if let Some(identy) = args.identy {
        builder = builder.identy(identy);
    }

    if let Some(contest) = args.contest {
        builder = builder.contest_path(contest);
    }

    if builder.have_server_url_override() {
        warn!(
            "overriding server_url requires that the server supports \
            HTTP/2.0, and is not recommended for normal use!"
        );
    }

    let mut cf = builder.build().unwrap_or_else(|e| {
        error!("can not build Codeforces client: {}", e);
        exit(1);
    });

    let dialect = args.dialect.as_deref();

    let logon = cf.probe_login_status().unwrap_or_else(|e| {
        error!("can not probe if we are already logon: {}", e);
        exit(1);
    });

    if !logon {
        // We are redirected.
        info!("authentication required");

        // Read password
        let prompt = format!("[cftool] password for {}: ", cf.get_identy());
        let passwd = rpassword::prompt_password(&prompt).unwrap_or_else(|err| {
            error!("failed reading password: {}", err);
            exit(1);
        });

        cf.login(&passwd).unwrap_or_else(|err| {
            error!("failed to login: {}", err);
            exit(1);
        });

        // Retry to GET the submit page.
        let logon = cf.probe_login_status().unwrap_or_else(|e| {
            error!("can not probe if we are already logon: {}", e);
            exit(1);
        });
        if !logon {
            error!(
                "authentication failed, maybe identy or password is\
                 wrong"
            );
            exit(1);
        }
    }

    match cf.maybe_save_cookie() {
        Err(e) => error!("cannot save cookie: {}", e),
        Ok(saved) => {
            if let Some(p) = saved {
                info!("cookie saved to {}", p.display());
            } else {
                info!("cookie not saved");
            }
        }
    }

    let problem = match action {
        Action::Submit(p) => p,
        Action::Dry => exit(0),
        Action::Query => {
            poll_or_query_verdict(&mut cf, need_poll, no_color);
            exit(0);
        }
        Action::None | Action::Err(_) => unreachable!(),
    };

    let source = args.source.unwrap();
    cf.submit(&problem, &source, dialect).unwrap_or_else(|err| {
        error!("submit failed: {}", err);
        exit(1);
    });

    if need_poll {
        poll_or_query_verdict(&mut cf, true, no_color);
    }
}
