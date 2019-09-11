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
        let re = Regex::new(r"a href.*submissionId=.(?P<id>[0-9]*).*<span class='verdict-(?P<verdict>.*)'>(?P<message>.*)</span></a>")
            .unwrap();
        let txt = resp.text()?;
        let caps = match re.captures(&txt) {
            Some(c) => c,
            None => return Err(Box::new(ParseError {})),
        };

        // Remove HTML labels like <span> from message
        let message = &caps["message"];
        let re = Regex::new(r"<.[^>]*>").unwrap();
        let clean_msg = re.replace_all(message, "");

        let id_msg = format!("{} {}", &caps["id"], clean_msg);

        Ok(match &caps["verdict"] {
            "accepted" => Verdict::Accepted(id_msg),
            "rejected" => Verdict::Rejected(id_msg),
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
