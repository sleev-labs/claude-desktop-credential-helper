use std::collections::BTreeMap;

#[derive(Debug)]
pub enum Cli {
    Run(Args),
    Version,
    Help,
}

#[derive(Debug, Default)]
pub struct Args {
    pub headers: BTreeMap<String, String>,
}

pub const USAGE: &str = "\
Usage: claude-desktop-cred [OPTIONS]

Prints the local Claude Code OAuth token in Claude Desktop's
inferenceCredentialHelper format: {\"token\": \"...\", \"headers\": {...}}.

Options:
      --header KEY=VALUE  Add a header to the printed headers object (repeatable)
      --version           Print the version
  -h, --help              Print this help";

pub fn parse(mut argv: impl Iterator<Item = String>) -> Result<Cli, String> {
    let mut args = Args::default();
    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--header" => {
                let Some(pair) = argv.next() else {
                    return Err("--header requires a KEY=VALUE argument".into());
                };
                let Some((key, value)) = pair.split_once('=') else {
                    return Err(format!("invalid --header '{pair}': expected KEY=VALUE"));
                };
                args.headers.insert(key.trim().to_owned(), value.to_owned());
            }
            "--version" => return Ok(Cli::Version),
            "-h" | "--help" => return Ok(Cli::Help),
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    Ok(Cli::Run(args))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_run(argv: &[&str]) -> Result<Args, String> {
        match parse(argv.iter().map(ToString::to_string))? {
            Cli::Run(args) => Ok(args),
            other => panic!("expected Cli::Run, got {other:?}"),
        }
    }

    #[test]
    fn parses_repeated_headers() {
        let args = parse_run(&["--header", "a=1", "--header", "b=x=y"]).unwrap();
        assert_eq!(args.headers["a"], "1");
        assert_eq!(args.headers["b"], "x=y");
    }

    #[test]
    fn defaults_to_no_headers() {
        assert!(parse_run(&[]).unwrap().headers.is_empty());
    }

    #[test]
    fn rejects_malformed_header() {
        assert!(parse_run(&["--header", "novalue"]).is_err());
        assert!(parse_run(&["--header"]).is_err());
    }

    #[test]
    fn rejects_unknown_argument() {
        assert!(parse_run(&["--frobnicate"]).is_err());
    }

    #[test]
    fn recognizes_version_and_help() {
        assert!(matches!(
            parse(["--version".to_owned()].into_iter()),
            Ok(Cli::Version)
        ));
        assert!(matches!(
            parse(["--help".to_owned()].into_iter()),
            Ok(Cli::Help)
        ));
    }
}
