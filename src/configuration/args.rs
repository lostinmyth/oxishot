use std::{io::IsTerminal, path::PathBuf};

pub enum Input {
    Path(PathBuf),
    Stdin,
}

pub struct Args {
    pub input: Input,
    pub output: Option<PathBuf>,
}

pub fn parse_args() -> Result<Args, lexopt::Error> {
    use lexopt::prelude::*;
    let mut input = None;
    let mut output = None;
    let mut p = lexopt::Parser::from_env();

    while let Some(arg) = p.next()? {
        match arg {
            Short('i') | Long("input") => input = Some(parse_input(p.value()?)),
            Short('o') | Long("output") => {
                output = Some(p.value()?.into());
            }
            Short('h') | Long("help") => {
                print_help(p.bin_name().unwrap_or("oxishot"));
                std::process::exit(0)
            }
            Short('v') | Long("version") => {
                println!("oxishot {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0)
            }
            Value(v) if input.is_none() => input = Some(parse_input(v)),
            _ => return Err(arg.unexpected()),
        }
    }

    let input = input.unwrap_or_else(|| {
        if !std::io::stdin().is_terminal() {
            Input::Stdin
        } else {
            print_help(p.bin_name().unwrap_or("oxishot"));
            std::process::exit(0)
        }
    });

    Ok(Args { input, output })
}

fn parse_input(v: std::ffi::OsString) -> Input {
    if v == "-" {
        Input::Stdin
    } else {
        Input::Path(v.into())
    }
}

fn print_help(bin: &str) {
    println!(
        "Usage: {bin} [OPTIONS] [FILE]

Open a screenshot for editing.

Arguments:
  [FILE]                 Image to open (\"-\" or omit to read from stdin)

Options:
  -i, --input <FILE>     Image to open, \"-\" for stdin
  -o, --output <FILE>    Where to save the result
  -h, --help             Print this help
  -v, --version          Print version"
    );
}
