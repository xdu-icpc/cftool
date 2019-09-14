use std::fmt;

pub struct Unescape<'a>(pub &'a str);

impl<'a> fmt::Display for Unescape<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Unescape(s) = *self;
        let pile_o_bits = s;
        let mut last = 0;
        let mut lastch = '#';
        for (i, ch) in s.bytes().enumerate() {
            match ch as char {
                '&' => {
                    fmt.write_str(&pile_o_bits[last..i])?;
                    last = i;
                    lastch = '&';
                }
                ';' => {
                    if lastch == '&' {
                        let s = match &pile_o_bits[last..=i] {
                            "&gt;" => ">",
                            "&lt;" => "<",
                            "&amp;" => "&",
                            "&#39;" => "'",
                            "&quot;" => "\"",
                            other => other,
                        };
                        fmt.write_str(s)?;
                    } else {
                        fmt.write_str(&pile_o_bits[last..=i])?;
                    }
                    last = i + 1;
                }
                _ => (),
            }
        }

        if last < s.len() {
            fmt.write_str(&pile_o_bits[last..])?;
        }
        Ok(())
    }
}
