use std::error::Error;
use std::path::Path;

// Copied from GNOME Epiphany-3.32.4.
fn user_agent() -> &'static str {
    return "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
            AppleWebkit/537.36 (KHTML, like Gecko) \
            Chrome/74.0.3729.169 \
            Safari/537.36";
}

pub struct Config {
    pub server_url: String,
    pub identy: String,
    pub contest_path: String,
    pub user_agent: String,
    pub prefer_cxx: String,
    pub prefer_py: String,
    pub debug: bool,
    pub no_cookie: bool,
    pub cookie: String,
}

impl Config {
    pub fn new() -> Self {
        Config {
            server_url: String::from("https://codeforces.com"),
            identy: String::from(""),
            contest_path: String::from(""),
            user_agent: String::from(user_agent()),
            prefer_cxx: String::from("c++17"),
            prefer_py: String::from("py3"),
            debug: false,
            no_cookie: false,
            cookie: String::from(""),
        }
    }

    // Override some config options from JSON config file.
    pub fn from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), Box<Error>> {
        use std::fs::File;
        use std::io::BufReader;
        let file = File::open(path)?;
        let rdr = BufReader::new(file);

        use serde_json::Value;
        let v: Value = serde_json::from_reader(rdr)?;

        // Stupid code.  Maybe need some refactoring.
        match &v["server_url"] {
            Value::String(s) => self.server_url = s.to_string(),
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

        match &v["debug"] {
            Value::Bool(b) => self.debug = *b,
            _ => (),
        };

        match &v["no_cookie"] {
            Value::Bool(b) => self.no_cookie = *b,
            _ => (),
        };

        Ok(())
    }
}
