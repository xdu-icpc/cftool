use error_chain::bail;

mod error {
    error_chain::error_chain! {}
}

use error::*;

pub fn cxx_dialect_recognize(d: &str) -> Result<&'static str> {
    Ok(match d {
        "c++11" | "cxx11" | "cpp11" | "c++0x" | "cxx0x" | "cpp0x" => "c++11",
        "c++14" | "cxx14" | "cpp14" | "c++1y" | "cxx1y" | "cpp1y" => "c++14",
        "c++17" | "cxx17" | "cpp17" | "c++1z" | "cxx1z" | "cpp1z" => "c++17",
        "c++-64" | "cxx-64" | "cpp-64" | "c++17-64" | "cxx17-64" | "cpp17-64" | "c++1z-64"
        | "cxx1z-64" | "cpp1z-64" => "c++17-64",
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

pub fn get_lang_dialect(dialect: &str) -> Result<&'static str> {
    Ok(match dialect {
        "c" => "43",
        "c++17-64" => "61",
        "c++17" => "54",
        "c++14" => "50",
        "c++11" => "42",
        "py3" => "31",
        "py2" => "7",
        "pypy3" => "41",
        "pypy2" => "40",
        "rust" => "49",
        "java" => "36",
        _ => bail!("don't know dialect {}", dialect),
    })
}

pub struct DialectParser {
    cxx_dialect: &'static str,
    py_dialect: &'static str,
}

impl DialectParser {
    pub fn new<T: AsRef<str>, U: AsRef<str>>(cxx_dialect: T, py_dialect: U) -> Result<Self> {
        Ok(Self {
            cxx_dialect: cxx_dialect_recognize(cxx_dialect.as_ref())?,
            py_dialect: py_dialect_recognize(py_dialect.as_ref())?,
        })
    }

    pub fn get_lang_ext(&self, ext: &str) -> Result<&'static str> {
        let dialect = match ext {
            "c" => "c",
            "cc" | "cp" | "cxx" | "cpp" | "CPP" | "c++" | "C" => self.cxx_dialect,
            "py" => self.py_dialect,
            "rs" => "rust",
            "java" => "java",
            _ => bail!("don't know extension {}", ext),
        };
        get_lang_dialect(dialect)
    }
}
