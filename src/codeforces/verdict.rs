mod error {
    error_chain::error_chain! {}
}

use error::*;
use error_chain::bail;

pub enum VerdictCode {
    Accepted,
    Rejected,
    Waiting,
    CompilationError,
}

pub struct Verdict {
    code: VerdictCode,
    id: String,
    msg: String,
}

impl Verdict {
    fn new<U: ToString, V: ToString>(code: VerdictCode, msg: U, id: V) -> Self {
        Verdict {
            code: code,
            msg: msg.to_string(),
            id: id.to_string(),
        }
    }

    pub fn parse(txt: &str) -> Result<Self> {
        use regex::Regex;
        use VerdictCode::*;

        // The line containing <td> mark is same in coach mode and normal
        // mode, but the next line containing the actual verdict is not.
        // So we have to match the previous line :(.
        let re = Regex::new(r"<td party[^>]* class=[^>]*status-verdict-cell.*submissionId=.(?P<id>[0-9]*).*\n(?P<line>.*)\n")
            .unwrap();
        let caps = match re.captures(txt) {
            Some(c) => c,
            None => bail!("no match for submission ID"),
        };
        let id = &caps["id"];
        let line = &caps["line"];

        if line.contains("Compilation error") {
            // Special case it because the CSS style is different.
            return Ok(Verdict::new(CompilationError, "Compilation error", id));
        }

        if line.contains("In queue") {
            // Likewise.
            return Ok(Verdict::new(Waiting, "In queue", id));
        }

        if line.contains("Pending judgement") {
            // Likewise.
            return Ok(Verdict::new(Waiting, "Pending judgement", id));
        }

        if line.contains("Partial") {
            // Likewise.
            return Ok(Verdict::new(Rejected, "Partial", id));
        }

        if line.contains("Skipped") {
            // Likewise.
            return Ok(Verdict::new(Rejected, "Skipped", id));
        }

        let re =
            regex::Regex::new(r"<span class='verdict-(?P<verdict>.*)'>(?P<message>.*)</").unwrap();
        let caps = match re.captures(&line) {
            Some(c) => c,
            None => bail!("no match for verdict"),
        };

        // Remove HTML labels like <span> from message
        let message = &caps["message"];
        let re = Regex::new(r"<.[^>]*>").unwrap();
        let clean_msg = re.replace_all(message, "");

        let code = match &caps["verdict"] {
            "accepted" => Accepted,
            "rejected" | "failed" => Rejected,
            "waiting" => Waiting,
            _ => bail!("unknown verdict {}", &caps["verdict"]),
        };

        Ok(Verdict::new(code, clean_msg, id))
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

        let msg = format!("{} {}", self.id, self.msg);

        w.write(msg.as_bytes())?;
        if use_color {
            w.reset()?;
        }
        w.write(b"\n")?;
        Ok(())
    }

    pub fn is_waiting(&self) -> bool {
        match self.code {
            VerdictCode::Waiting => true,
            _ => false,
        }
    }

    pub fn is_compilation_error(&self) -> bool {
        match self.code {
            VerdictCode::CompilationError => true,
            _ => false,
        }
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }
}
