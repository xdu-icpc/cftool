use std::error::Error;

#[derive(Debug)]
struct ParseError;

impl std::error::Error for ParseError {}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "can not parse my submission page")
    }
}

pub enum Verdict {
    Accepted(String),
    Rejected(String),
    Waiting(String),
}

impl Verdict {
    pub fn parse(resp: &mut reqwest::Response) -> Result<Self, Box<Error>> {
        use regex::Regex;

        // The line containing <td> mark is same in coach mode and normal
        // mode, but the next line containing the actual verdict is not.
        // So we have to match the previous line :(.
        let re = Regex::new(r"<td party[^>]* class=[^>]*status-verdict-cell.*submissionId=.(?P<id>[0-9]*).*\n(?P<line>.*)\n")
            .unwrap();
        let txt = resp.text()?;
        let caps = match re.captures(&txt) {
            Some(c) => c,
            None => return Err(Box::new(ParseError {})),
        };
        let id = &caps["id"];
        let line = &caps["line"];

        if line.contains("Compilation error") {
            // Special case it because the CSS style is different.
            let id_msg = format!("{}: Compilation error", id);
            return Ok(Verdict::Rejected(id_msg));
        }

        if line.contains("In queue") {
            // Likewise.
            let id_msg = format!("{}: In queue", id);
            return Ok(Verdict::Waiting(id_msg));
        }

        if line.contains("Pending judgement") {
            // Likewise.
            let id_msg = format!("{}: Pending judgement", id);
            return Ok(Verdict::Waiting(id_msg));
        }

        if line.contains("Partial") {
            // Likewise.
            let id_msg = format!("{}: Partial", id);
            return Ok(Verdict::Rejected(id_msg));
        }

        if line.contains("Skipped") {
            // Likewise.
            let id_msg = format!("{}: Skipped", id);
            return Ok(Verdict::Rejected(id_msg));
        }

        let re =
            regex::Regex::new(r"<span class='verdict-(?P<verdict>.*)'>(?P<message>.*)</").unwrap();
        let caps = match re.captures(&line) {
            Some(c) => c,
            None => return Err(Box::new(ParseError {})),
        };

        // Remove HTML labels like <span> from message
        let message = &caps["message"];
        let re = Regex::new(r"<.[^>]*>").unwrap();
        let clean_msg = re.replace_all(message, "");

        let id_msg = format!("{} {}", id, clean_msg);

        Ok(match &caps["verdict"] {
            "accepted" => Verdict::Accepted(id_msg),
            "rejected" | "failed" => Verdict::Rejected(id_msg),
            "waiting" => Verdict::Waiting(id_msg),
            _ => return Err(Box::new(ParseError {})),
        })
    }

    pub fn print(&self, w: &mut termcolor::WriteColor) -> std::io::Result<()> {
        use termcolor::Color::{Green, Red};
        use termcolor::ColorSpec;
        let use_color = w.supports_color();
        if use_color {
            let color = match self {
                Verdict::Accepted(_) => Some(Green),
                Verdict::Rejected(_) => Some(Red),
                Verdict::Waiting(_) => None,
            };
            w.set_color(ColorSpec::new().set_fg(color))?;
        }

        let msg = match self {
            Verdict::Accepted(s) => s,
            Verdict::Rejected(s) => s,
            Verdict::Waiting(s) => s,
        };

        w.write(msg.as_bytes())?;
        if use_color {
            w.reset()?;
        }
        w.write(b"\n")?;
        Ok(())
    }
}
