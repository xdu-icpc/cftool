use reqwest::StatusCode;
use url::Url;

mod error {
    error_chain::error_chain! {}
}

use error::*;

#[derive(Debug)]
pub enum Response {
    Content(String),
    Redirection(Url),
    Other(StatusCode),
}

impl TryFrom<reqwest::blocking::Response> for Response {
    type Error = Error;
    fn try_from(resp: reqwest::blocking::Response) -> Result<Response> {
        if resp.status().is_success() {
            return Ok(Self::Content(
                resp.text().chain_err(|| "cannot parse response body")?,
            ));
        }

        if resp.status().is_redirection() {
            let url_str = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .chain_err(|| "no LOCATION")?
                .to_str()
                .chain_err(|| "can not parse LOCATION")?;
            return Ok(Self::Redirection(
                Url::parse(url_str).chain_err(|| "can not parse LOCATION as URL")?,
            ));
        }

        Ok(Self::Other(resp.status()))
    }
}
