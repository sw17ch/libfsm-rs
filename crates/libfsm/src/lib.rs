use libfsm_api::Fsm;
use litrs::Literal;
use proc_macro::{TokenStream, TokenTree};

#[derive(Default)]
enum PcreArgParser {
    #[default]
    NeedName,
    NeedComma {
        name: String,
    },
    NeedPattern {
        name: String,
    },
    Done {
        name: String,
        pattern: Vec<u8>,
    },
}

impl PcreArgParser {
    fn push(&mut self, tt: TokenTree) -> Result<(), String> {
        *self = match std::mem::take(self) {
            PcreArgParser::NeedName => {
                if matches!(&tt, TokenTree::Ident(_)) {
                    PcreArgParser::NeedComma {
                        name: tt.to_string(),
                    }
                } else {
                    return Err(format!("expected ident, got: {tt:?}"));
                }
            }
            PcreArgParser::NeedComma { name } => {
                if matches!(&tt, TokenTree::Punct(p) if p.as_char() == ',') {
                    PcreArgParser::NeedPattern { name }
                } else {
                    return Err(format!("expected comma (,), got: {tt:?}"));
                }
            }
            PcreArgParser::NeedPattern { name } => {
                let pattern = match Literal::try_from(tt) {
                    Ok(Literal::String(s)) => s.into_value().as_bytes().to_vec(),
                    Ok(Literal::ByteString(s)) => s.into_value().to_vec(),
                    Ok(other_lit) => {
                        return Err(format!("expected string literal, got: {other_lit:?}"));
                    }
                    Err(e) => {
                        return Err(format!("invalid literal token: {e:?}"));
                    }
                };

                PcreArgParser::Done { name, pattern }
            }
            PcreArgParser::Done { .. } => {
                return Err(format!("unexpected input after pattern: {tt:?}"));
            }
        };
        Ok(())
    }

    fn finish(self) -> Option<(String, Vec<u8>)> {
        let Self::Done { name, pattern } = self else {
            return None;
        };
        let name = name.to_string();
        Some((name, pattern))
    }
}

#[proc_macro]
pub fn pcre(ts: TokenStream) -> TokenStream {
    let mut p = PcreArgParser::default();
    for t in ts.clone() {
        if let Err(e) = p.push(t) {
            panic!("unexpected token: {e:?}");
        }
    }
    let Some((name, pattern)) = p.finish() else {
        panic!("expected 'name, pattern'. got: {ts:?}");
    };

    let mut fsm = match Fsm::compile_pcre(pattern.into_iter()) {
        Ok(fsm) => fsm,
        Err(e) => panic!("compiling fsm failed: {e:?}"),
    };
    let code = match fsm.print() {
        Ok(c) => c,
        Err(e) => panic!("generating code from fsm failed: {e:?}"),
    };
    let code = String::from_utf8(code).expect("code is utf-8");
    let wrapped_code =
        format!("mod {name}_module {{ {code} }} pub use {name}_module::fsm_main as {name};");
    wrapped_code.parse().expect("code is not rust")
}
