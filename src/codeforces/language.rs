use error_chain::bail;

mod error {
    error_chain::error_chain! {}
}

use error::*;

#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Dialect {
    C = 43,
    CXX20 = 73,
    CXX17_64 = 61,
    CXX17 = 54,
    CXX14 = 50,
    Python3 = 31,
    Python2 = 7,
    Pypy3 = 41,
    Pypy2 = 40,
    Rust2021 = 75,
    Java = 36,
}

pub fn cxx_dialect_recognize(d: &str) -> Result<Dialect> {
    use Dialect::*;
    Ok(match d {
        "c++14" | "cxx14" | "cpp14" | "c++1y" | "cxx1y" | "cpp1y" => CXX14,
        "c++17" | "cxx17" | "cpp17" | "c++1z" | "cxx1z" | "cpp1z" => CXX17,
        "c++17-64" | "cxx17-64" | "cpp17-64" | "c++1z-64" | "cxx1z-64" | "cpp1z-64" => CXX17_64,
        "c++20" | "cxx20" | "cpp20" | "c++2a" | "cxx2a" | "cpp2a" => CXX20,
        "c++20-64" | "cxx20-64" | "cpp20-64" | "c++2a-64" | "cxx2a-64" | "cpp2a-64" => CXX20,
        "c++11" | "cxx11" | "cpp11" | "c++1x" | "cxx1x" | "cpp1x" => {
            bail!("C++11 support has been removed by Codeforces")
        }
        _ => bail!("unknown or unsupported C++ dialect: {}", d),
    })
}

pub fn py_dialect_recognize(d: &str) -> Result<Dialect> {
    use Dialect::*;
    Ok(match d {
        "py2" | "python2" | "cpython2" => Python2,
        "py3" | "python3" | "cpython3" => Python3,
        "pypy2" => Pypy2,
        "pypy3" => Pypy3,
        _ => bail!("unknown or unsupported Python dialect: {}", d),
    })
}

pub fn rs_edition_recognize(e: &str) -> Result<Dialect> {
    Ok(match e {
        "2021" => Dialect::Rust2021,
        _ => bail!("unknown or unsupported Rust edition: {}", e),
    })
}

impl Dialect {
    pub fn new<S: AsRef<str>>(s: S) -> Result<Self> {
        use Dialect::*;
        Ok(match s.as_ref() {
            "c" => C,
            "c++20" => CXX20,
            "c++17-64" => CXX17_64,
            "c++17" => CXX17,
            "c++14" => CXX14,
            "py3" => Python3,
            "py2" => Python2,
            "pypy3" => Pypy3,
            "pypy2" => Pypy2,
            "rust2021" => Rust2021,
            "java" => Java,
            _ => bail!("don't know dialect {}", s.as_ref()),
        })
    }
    pub fn to_id(self) -> String {
        (self as u32).to_string()
    }
}

pub struct DialectParser {
    cxx_dialect: Dialect,
    py_dialect: Dialect,
    rs_edition: Dialect,
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

    pub fn get_lang_ext(&self, ext: &str) -> Result<Dialect> {
        Ok(match ext {
            "c" => Dialect::C,
            "cc" | "cp" | "cxx" | "cpp" | "CPP" | "c++" | "C" => self.cxx_dialect,
            "py" => self.py_dialect,
            "rs" => self.rs_edition,
            "java" => Dialect::Java,
            _ => bail!("don't know extension {}", ext),
        })
    }
}
