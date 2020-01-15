use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub server_url: Option<String>,
    pub identy: Option<String>,
    pub contest_path: Option<String>,
    pub user_agent: Option<String>,
    pub prefer_cxx: Option<String>,
    pub prefer_py: Option<String>,
    pub cookie_file: Option<String>,
    pub retry_limit: Option<i64>,
    pub no_cookie: Option<bool>,
}
