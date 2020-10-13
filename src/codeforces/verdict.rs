use serde_aux::field_attributes::deserialize_bool_from_anything;

mod error {
    error_chain::error_chain! {}
}

use error::*;

pub enum VerdictCode {
    Accepted,
    Rejected,
    Waiting,
    CompilationError,
}

pub struct Verdict {
    code: VerdictCode,
    msg: String,
}

pub fn parse_submission_id(txt: &str) -> Result<String> {
    use regex::Regex;
    let re = Regex::new(
        r"<td party[^>]* class=[^>]*status-verdict-cell.*submissionId=.(?P<id>[0-9]*).*\n",
    )
    .unwrap();
    let caps = re
        .captures(txt)
        .chain_err(|| "no match for submission ID")?;
    Ok(caps["id"].to_owned())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerdictJson {
    #[serde(deserialize_with = "deserialize_bool_from_anything")]
    compilation_error: bool,
    verdict: String,
    #[serde(deserialize_with = "deserialize_bool_from_anything")]
    waiting: bool,
}

impl Verdict {
    fn new<T: ToString>(code: VerdictCode, msg: T) -> Self {
        Verdict {
            code,
            msg: msg.to_string(),
        }
    }

    pub fn from_json(json: &str) -> Result<Self> {
        use regex::Regex;
        use VerdictCode::*;

        let verdict_json: VerdictJson =
            serde_json::from_str(json).chain_err(|| "can not parse JSON")?;

        // Remove HTML labels like <span> from message
        let re = Regex::new(r"<.[^>]*>").unwrap();
        let msg = re.replace_all(&verdict_json.verdict, "");

        if verdict_json.compilation_error {
            return Ok(Verdict::new(CompilationError, msg));
        }

        if verdict_json.waiting {
            return Ok(Verdict::new(Waiting, msg));
        }

        if verdict_json.verdict.contains("verdict-accepted") {
            return Ok(Verdict::new(Accepted, msg));
        }

        Ok(Verdict::new(Rejected, msg))
    }

    pub fn print<W: termcolor::WriteColor>(&self, w: &mut W) -> std::io::Result<()> {
        use termcolor::Color::{Green, Red};
        use termcolor::ColorSpec;
        use VerdictCode::*;
        let use_color = w.supports_color();
        if use_color {
            let color = match &self.code {
                Accepted => Some(Green),
                Rejected | CompilationError => Some(Red),
                Waiting => None,
            };
            w.set_color(ColorSpec::new().set_fg(color))?;
        }

        w.write_all(self.msg.as_bytes())?;
        if use_color {
            w.reset()?;
        }
        w.write_all(b"\n")?;
        Ok(())
    }

    pub fn is_waiting(&self) -> bool {
        matches!(self.code, VerdictCode::Waiting)
    }

    pub fn is_compilation_error(&self) -> bool {
        matches!(self.code, VerdictCode::CompilationError)
    }
}
