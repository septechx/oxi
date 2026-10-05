use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Error, Result, anyhow};
use clap::Parser;

use crate::driver::UnprettyPrintable;

#[derive(Debug, Clone, Copy, Default)]
pub enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

impl FromStr for ColorChoice {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(ColorChoice::Auto),
            "always" => Ok(ColorChoice::Always),
            "never" => Ok(ColorChoice::Never),
            other => Err(anyhow!("invalid color choice: {}", other)),
        }
    }
}

impl FromStr for UnprettyPrintable {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tokens" => Ok(UnprettyPrintable::Tokens),
            "ast" => Ok(UnprettyPrintable::Ast),
            "resolver" => Ok(UnprettyPrintable::Resolver),
            "hir" => Ok(UnprettyPrintable::Hir),
            "typeck" => Ok(UnprettyPrintable::Typeck),
            "thir" => Ok(UnprettyPrintable::Thir),
            other => Err(anyhow!("invalid unpretty option: {}", other)),
        }
    }
}

#[derive(Parser, Debug)]
#[clap(version, about, long_about = None, arg_required_else_help(true))]
pub struct Cli {
    #[clap(required = true)]
    pub input: PathBuf,

    #[clap(short, long)]
    pub output: Option<PathBuf>,

    #[clap(
        long,
        help = "When to use colors [possible values: auto, always, never]",
        default_value = "auto"
    )]
    pub color: ColorChoice,

    #[clap(long, help = "Print intermediate representation")]
    pub unpretty: Option<UnprettyPrintable>,

    #[clap(long, help = "Do not print any output")]
    pub quiet: bool,

    #[clap(
        long = "Dcpu",
        help = "Select a CPU architecture to target",
        default_value = "x86-64"
    )]
    pub cpu: String,

    #[clap(
        long = "Dfeatures",
        help = "Select a feature set to enable",
        default_value = "+avx2"
    )]
    pub features: String,

    #[clap(long = "no-pie", help = "Disable position independent executable")]
    pub no_pie: bool,

    #[clap(long = "no-pic", help = "Disable position independent code")]
    pub no_pic: bool,

    #[clap(long = "shared", help = "Generate a shared library")]
    pub shared: bool,

    #[clap(long = "static", help = "Generate a static library")]
    pub static_: bool,

    #[clap(long = "strip", help = "Strip symbols from executable")]
    pub strip: bool,
}
