use std::{path::PathBuf, str::FromStr};

use anyhow::{Error, Result, anyhow};
use clap::Parser;
use inkwell::OptimizationLevel;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum OptLevel {
    O0,
    O1,
    O2,
    O3,
}

impl FromStr for OptLevel {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "0" => Ok(OptLevel::O0),
            "1" => Ok(OptLevel::O1),
            "2" => Ok(OptLevel::O2),
            "3" => Ok(OptLevel::O3),
            other => Err(anyhow!("invalid optimization level: {}", other)),
        }
    }
}

impl From<OptLevel> for OptimizationLevel {
    fn from(level: OptLevel) -> OptimizationLevel {
        match level {
            OptLevel::O0 => OptimizationLevel::None,
            OptLevel::O1 => OptimizationLevel::Less,
            OptLevel::O2 => OptimizationLevel::Default,
            OptLevel::O3 => OptimizationLevel::Aggressive,
        }
    }
}

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

#[derive(Debug, Clone, Copy)]
pub enum EmitOption {
    Asm,
    LlvmIr,
    None,
}

impl FromStr for EmitOption {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "asm" => Ok(EmitOption::Asm),
            "llvm-ir" => Ok(EmitOption::LlvmIr),
            "none" => Ok(EmitOption::None),
            other => Err(anyhow!("invalid emit option: {}", other)),
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
        help = "Output IR [possible values: llvm-ir, asm, none]",
        default_value = "none"
    )]
    pub emit_ir: EmitOption,

    #[clap(
        long,
        help = "When to use colors [possible values: auto, always, never]",
        default_value = "auto"
    )]
    pub color: ColorChoice,

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

    #[clap(
        short = 'O',
        long = "Doptimize",
        help = "Set optimization level [possible values: 0, 1, 2, 3]",
        default_value = "3"
    )]
    pub opt: OptLevel,

    #[clap(long = "no-pie", help = "Disable position independent executable")]
    pub no_pie: bool,

    #[clap(long = "no-pic", help = "Disable position independent code")]
    pub no_pic: bool,

    #[clap(long = "shared", help = "Generate a shared library")]
    pub shared: bool,

    #[clap(long = "static", help = "Generate a static library")]
    pub static_: bool,

    #[clap(long = "no-link", help = "Only compile, do not link")]
    pub no_link: bool,

    #[clap(long = "strip", help = "Strip symbols from executable")]
    pub strip: bool,

    #[clap(long = "gcc", help = "Use gcc as a linker instead of mold/lld/gold/ld")]
    pub use_gcc: bool,
}
