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
}

fn main() {
    const VERSION: &str =
        git_version::git_version!(args = ["--tags", "--always", "--dirty=-modified"]);
    use clap::{App, Arg};
    let matches = App::new("XDU-ICPC cftool")
        .version(VERSION)
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
    let modules = &[module_path!(), "reqwest"];
    stderrlog::new()
        .modules(modules.iter().cloned())
        .verbosity(v + 1)
        .init()
        .unwrap();

    info!("this is XDU-ICPC cftool, {}", VERSION);

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

    let source = matches.value_of("source").unwrap_or("");
    if matches.value_of("source").is_some() {
        match action {
            Action::Dry | Action::Query => {
                error!(
                    "specifying source code file does not make sense \
                    without submitting it"
                );
                exit(1);
            }
            Action::Submit(_) => (),
            Action::None => {
                let path = std::path::Path::new(source);
                if let Some(s) = path.file_stem().and_then(|x| x.to_str()) {
                    if s.len() == 1 {
                        let s = s.to_owned();
                        action = match s.chars().next().unwrap() {
                            'A'..='Z' => Action::Submit(s),
                            'a'..='z' => Action::Submit(s.to_uppercase()),
                            _ => Action::None,
                        }
                    }
                }
                if let Action::Submit(problem) = &action {
                    info!("guessed problem ID to be {}", problem);
                }
            }
        }
    }

    if need_poll {
        if let Action::None = action {
            action = Action::Query;
        }
    }

    if let Action::None = action {
        error!("must use one of --dry-run, --query, and --problem");
        exit(1);
    }

    if let Action::Submit(_) = &action {
        if matches.value_of("source").is_none() {
            error!("attempt to submit, but no source code specified");
            exit(1);
        }
    }

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

    let dialect = matches.value_of("dialect");

    let logon = cfg.probe_login_status().unwrap_or_else(|e| {
        error!("can not probe if we are already logon: {}", e);
        exit(1);
    });

    if !logon {
        // We are redirected.
        info!("authentication required");

        // Read password
        let prompt = format!("[cftool] password for {}: ", cfg.get_identy());
        let passwd = rpassword::prompt_password_stderr(&prompt).unwrap_or_else(|err| {
            error!("failed reading password: {}", err);
            exit(1);
        });

        cfg.login(&passwd).unwrap_or_else(|err| {
            error!("failed to login: {}", err);
            exit(1);
        });

        // Retry to GET the submit page.
        let logon = cfg.probe_login_status().unwrap_or_else(|e| {
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

    match cfg.maybe_save_cookie() {
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
            poll_or_query_verdict(&mut cfg, false, no_color);
            exit(0);
        }
        Action::None => unreachable!(),
    };

    cfg.submit(&problem, source, dialect).unwrap_or_else(|err| {
        error!("submit failed: {}", err);
        exit(1);
    });

    if need_poll {
        poll_or_query_verdict(&mut cfg, true, no_color);
    }
}
