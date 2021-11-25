use error_chain::bail;

mod error {
    error_chain::error_chain! {}
}

use error::*;

pub fn cxx_dialect_recognize(d: &str) -> Result<&'static str> {
    Ok(match d {
        "c++14" | "cxx14" | "cpp14" | "c++1y" | "cxx1y" | "cpp1y" => "c++14",
        "c++17" | "cxx17" | "cpp17" | "c++1z" | "cxx1z" | "cpp1z" => "c++17",
        "c++17-64" | "cxx17-64" | "cpp17-64" | "c++1z-64" | "cxx1z-64" | "cpp1z-64" => "c++17-64",
        "c++20" | "cxx20" | "cpp20" | "c++2a" | "cxx2a" | "cpp2a" => "c++20",
        "c++20-64" | "cxx20-64" | "cpp20-64" | "c++2a-64" | "cxx2a-64" | "cpp2a-64" => "c++20",
        "c++11" | "cxx11" | "cpp11" | "c++1x" | "cxx1x" | "cpp1x" => {
            bail!("C++11 support has been removed by Codeforces")
        }
        _ => bail!("unknown or unsupported C++ dialect: {}", d),
    })
}

pub fn py_dialect_recognize(d: &str) -> Result<&'static str> {
    Ok(match d {
        "py2" | "python2" | "cpython2" => "py2",
        "py3" | "python3" | "cpython3" => "py3",
        "pypy2" => "pypy2",
        "pypy3" => "pypy3",
        _ => bail!("unknown or unsupported Python dialect: {}", d),
    })
}

pub fn rs_edition_recognize(e: &str) -> Result<&'static str> {
    Ok(match e {
        "2018" => "rust2018",
        "2021" => "rust2021",
        _ => bail!("unknown or unsupported Rust edition: {}", e),
    })
}

pub fn get_lang_dialect(dialect: &str) -> Result<&'static str> {
    Ok(match dialect {
        "c" => "43",
        "c++20" => "73",
        "c++17-64" => "61",
        "c++17" => "54",
        "c++14" => "50",
        "py3" => "31",
        "py2" => "7",
        "pypy3" => "41",
        "pypy2" => "40",
        "rust2018" => "49",
        "rust2021" => "75",
        "java" => "36",
        _ => bail!("don't know dialect {}", dialect),
    })
}

pub struct DialectParser {
    cxx_dialect: &'static str,
    py_dialect: &'static str,
    rs_edition: &'static str,
}

impl DialectParser {
    pub fn new<T: AsRef<str>, U: AsRef<str>, V: AsRef<str>>(
        cxx_dialect: T,
        py_dialect: U,
        rs_edition: V,
    ) -> Result<Self> {
        Ok(Self {
            cxx_dialect: cxx_dialect_recognize(cxx_dialect.as_ref())?,
            py_dialect: py_dialect_recognize(py_dialect.as_ref())?,
            rs_edition: rs_edition_recognize(rs_edition.as_ref())?,
        })
    }

    pub fn get_lang_ext(&self, ext: &str) -> Result<&'static str> {
        let dialect = match ext {
            "c" => "c",
            "cc" | "cp" | "cxx" | "cpp" | "CPP" | "c++" | "C" => self.cxx_dialect,
            "py" => self.py_dialect,
            "rs" => self.rs_edition,
            "java" => "java",
            _ => bail!("don't know extension {}", ext),
        };
        get_lang_dialect(dialect)
    }
}
