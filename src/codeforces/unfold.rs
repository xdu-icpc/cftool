// Preprocessor to unfold the source into one file

use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot parse file: {0}")]
    Parse(syn::parse::Error),
    #[error("input/output error: {0}")]
    IO(std::io::Error),
    #[error("multiple path attribute for a mod")]
    MultiplePathAttr,
    #[error("bad path attribute: expect {0}")]
    BadPathAttr(&'static str),
    #[error("it seems {0} is not a valid path to source file")]
    BadSrcPath(PathBuf),
    #[error("module found at both {0} and {1}")]
    AmbiguityModule(PathBuf, PathBuf),
    #[error("rustfmt fail")]
    Rustfmt,
}

pub type Result<T> = std::result::Result<T, Error>;

fn unfold_rust_src_recursive<P: AsRef<Path>>(p: P, search_parent: bool) -> Result<syn::File> {
    let p = PathBuf::from(p.as_ref());
    let mut file = std::fs::File::open(&p).map_err(Error::IO)?;
    let mut content = String::new();
    let parent = p
        .parent()
        .ok_or_else(|| Error::BadSrcPath(PathBuf::from(&p)))?;
    let mut recursive_sp = false;

    use std::io::Read;
    file.read_to_string(&mut content).map_err(Error::IO)?;
    let mut ast = syn::parse_file(&content).map_err(Error::Parse)?;

    let mut items = vec![];
    std::mem::swap(&mut ast.items, &mut items);

    for mut item in items {
        if let syn::Item::Mod(m) = &mut item {
            let mut path_attr_idx = None;
            for i in 0..m.attrs.len() {
                if m.attrs[i].path.is_ident("path") {
                    if path_attr_idx.is_some() {
                        return Err(Error::MultiplePathAttr);
                    }
                    path_attr_idx = Some(i);
                }
            }

            let mod_path = path_attr_idx
                .map(|x| {
                    let attr = m.attrs.swap_remove(x);
                    let mut it = attr.tokens.into_iter();
                    use proc_macro2::TokenTree;

                    let msg = "'=' after 'path'";

                    if let Some(TokenTree::Punct(punct)) = it.next() {
                        if punct.as_char() != '=' {
                            return Err(Error::BadPathAttr(msg));
                        }
                    } else {
                        return Err(Error::BadPathAttr(msg));
                    }

                    let msg = "a string literal after '='";

                    let str_lit: litrs::StringLit<String> = it
                        .next()
                        .ok_or(Error::BadPathAttr(msg))?
                        .try_into()
                        .map_err(|_| Error::BadPathAttr(msg))?;

                    Ok(parent.join(str_lit.into_value().as_ref()))
                })
                .transpose()?
                .map(|x| {
                    recursive_sp = true;
                    Ok(x)
                })
                .unwrap_or_else(|| {
                    let mod_name = m.ident.to_string();

                    let search_dir = if search_parent {
                        parent.to_owned()
                    } else {
                        parent.join(p.file_stem().unwrap())
                    };

                    let p1 = search_dir.join(&mod_name).join("mod.rs");
                    let p2 = search_dir.join(mod_name + ".rs");

                    if p1.exists() && p2.exists() {
                        return Err(Error::AmbiguityModule(p1, p2));
                    }

                    if p1.exists() {
                        recursive_sp = true;
                        return Ok(p1);
                    }

                    Ok(p2)
                })?;

            let mod_file = unfold_rust_src_recursive(mod_path, recursive_sp)?;
            use syn::token::Brace;
            m.content = Some((Brace::default(), mod_file.items));
        }

        ast.items.push(item);
    }

    Ok(ast)
}

fn run_rustfmt(content: &str) -> Result<String> {
    use std::process::{Command, Stdio};
    use Error::Rustfmt;
    let content = content.to_owned();

    let mut rustfmt = Command::new("rustfmt")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|_| Rustfmt)?;

    let mut stdin = rustfmt.stdin.take().ok_or(Rustfmt)?;
    std::thread::spawn(move || {
        use std::io::Write;
        stdin.write_all(content.as_bytes()).unwrap();
    });

    let output = rustfmt.wait_with_output().map_err(|_| Rustfmt)?;
    String::from_utf8(output.stdout).map_err(|_| Rustfmt)
}

pub fn unfold_rust<P: AsRef<Path>>(p: P) -> Result<String> {
    unfold_rust_src_recursive(p, true).map(|ast| {
        use quote::ToTokens;
        let content = ast.into_token_stream().to_string();
        run_rustfmt(&content).unwrap_or(content)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unfold_rust() {
        let x = unfold_rust("example/t.rs").unwrap();
        assert_eq!(
            x,
            "mod a {
    pub mod c {
        pub fn f() -> i32 {
            42
        }
    }
}
mod b {
    pub mod c {
        pub fn f() -> i32 {
            47
        }
    }
}
fn main() {
    println!(\"{}\", a::c::f() + b::c::f());
}
"
        );
    }
}
