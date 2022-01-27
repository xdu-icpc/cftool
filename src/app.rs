#![deny(missing_docs)]

pub use clap::Parser;

/// Version automatically generated from git
pub const VERSION: &str =
    git_version::git_version!(args = ["--tags", "--always", "--dirty=-modified"]);

/// A command line tool for submitting code to Codeforces
#[derive(Parser, Debug)]
#[clap(author, version = VERSION)]
pub struct App {
    /// Performs authentication and exit
    #[clap(short, long)]
    pub dry_run: bool,

    /// Disables color for verdict
    #[clap(short = 'w', long)]
    pub no_color: bool,

    /// Polls the last submission until it's judged
    #[clap(short = 'l', long)]
    pub poll: bool,

    /// Queries the status of the last submission in the contest
    #[clap(short = 'q', long)]
    pub query: bool,

    /// Sets the level of verbosity
    #[clap(short = 'v', parse(from_occurrences))]
    pub verbose: usize,

    /// Sets a custom config file, overriding other config files
    #[clap(short = 'c', long)]
    pub config: Option<String>,

    /// Sets a contest path, overriding the config files
    #[clap(short = 'o', long)]
    pub contest: Option<String>,

    /// Sets a cookie cache file path, overriding the default
    #[clap(short = 'k', long)]
    pub cookie: Option<String>,

    /// Sets the language dialect, overriding config and filename
    #[clap(short = 'a', long)]
    pub dialect: Option<String>,

    /// Sets the identy (handle or email), overriding the config files
    #[clap(short = 'i', long)]
    pub identy: Option<String>,

    /// Sets the problem ID to be submitted for
    #[clap(short = 'p', long)]
    pub problem: Option<String>,

    /// Sets the server URL, overriding the config files
    #[clap(short = 'u', long)]
    pub server: Option<String>,

    /// Submits this source code file
    #[clap(short = 's', long)]
    pub source: Option<String>,

    /// Bypass the sanity check for problem ID
    #[clap(short, long)]
    pub force: bool,
}
