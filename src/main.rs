// Copyright ⓒ  His Majesty the King in Right of Canada, as
// represented by the Minister of Natural Resources, 2026
// Licensed under the MIT license
// (see LICENSE or <http://opensource.org/licenses/MIT>) All files in the project carrying such
// notice may not be copied, modified, or distributed except according to those terms.

extern crate simple_error;
#[macro_use]
extern crate env_var;
extern crate dirs;
use clap::{Arg, ArgAction, Command};
use log::{error, LevelFilter};
use log4rs::append::console::ConsoleAppender;
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::Config;
use regex::Regex;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::str::FromStr;
extern crate ini;

use eodms_data_downloader::{download_files, normalize_url, Conf, RunMode, URL_LUT};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let conf = process_args()?;
    download_files(&conf)?;
    Ok(())
}

#[allow(clippy::too_many_lines)] // temporary until refactor
fn process_args() -> Result<Conf, Box<dyn Error>> {
    let env_mode = env_var!(optional "MODE", default: "Prod");
    let run_mode = RunMode::from_str(env_mode.as_str())? as usize;
    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %T%.3f)(utc)} [{l}] - {m}{n}",
        )))
        .build();
    let filter = match run_mode {
        0 => LevelFilter::Debug,
        _ => LevelFilter::Info,
    };

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .build(Root::builder().appender("stdout").build(filter))
        .unwrap();
    let loghandle = log4rs::init_config(config).unwrap();
    let matches = Command::new("eodms-downloader")
        .about("Utility to download directories and files from EODMS Data Server")
        .version("1.0.0")
        .subcommand_required(false)
        .arg_required_else_help(true)
        .author("EODMS Development Team")
        .arg(
            Arg::new("url")
                .short('u')
                .long("url")
                .help("")
                .action(ArgAction::Set)
                .required(true)
                .num_args(1),
        )
        .arg(
            Arg::new("out")
                .short('o')
                .long("outdir")
                .help("output directory")
                .action(ArgAction::Set)
                .required(true)
                .num_args(1),
        )
        .arg(
            Arg::new("rec")
                .short('r')
                .long("rec")
                .help("recursive")
                .required(false)
                .num_args(0)
                .value_parser(clap::builder::BoolishValueParser::new()),
        )
        .arg(
            Arg::new("inc")
                .short('i')
                .long("include")
                .help("include regex")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("threads")
                .short('t')
                .long("threads")
                .value_parser(clap::value_parser!(u8))
                .help("number of download threads")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("loglevel")
                .short('l')
                .long("loglevel")
                .help("logging level off, error, info, debug, trace")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("stripdirs")
                .short('s')
                .long("stripdirs")
                .help("strip the leading directories")
                .num_args(0)
                .value_parser(clap::builder::BoolishValueParser::new()),
        )
        .get_matches();
    let mut conf = Conf::default();
    if let Some(out) = matches.get_one::<String>("out") {
        let path = Path::new(out);
        if path.exists() {
            if !path.is_dir() {
                error!("{} exists and is not a directory", path.display());
                let err = format!("{} exists and is not a directory", path.display());
                return Err(err.as_str().into());
            }
        } else {
            fs::create_dir(out)?;
        }
        conf.output_directory.clone_from(out);
    }
    if let Some(incrx) = matches.get_one::<String>("inc") {
        let re = Regex::new(incrx)?;
        conf.incrx = Some(re);
    }
    if let Some(recursive) = matches.get_one::<bool>("rec") {
        conf.recursive = *recursive;
    }
    if let Some(stripdirs) = matches.get_one::<bool>("stripdirs") {
        conf.stripdirs = *stripdirs;
    }
    if let Some(threads) = matches.get_one::<u8>("threads") {
        conf.num_threads = *threads as usize;
    }
    if let Some(loglevel) = matches.get_one::<String>("loglevel") {
        let lfilter = match loglevel.as_str() {
            "off" => LevelFilter::Off,
            "error" => LevelFilter::Error,
            "warn" => LevelFilter::Warn,
            "info" => LevelFilter::Info,
            "debug" => LevelFilter::Debug,
            "trace" => LevelFilter::Trace,
            &_ => {
                return Err(
                    "Invalid loglevel, needs to be one of: off,error,warn,info,debug or trace"
                        .into(),
                )
            }
        };
        let stdout = ConsoleAppender::builder()
            .encoder(Box::new(PatternEncoder::new(
                "{d(%Y-%m-%d %T%.3f)(utc)} [{l}] - {m}{n}",
            )))
            .build();
        let config = Config::builder()
            .appender(Appender::builder().build("stdout", Box::new(stdout)))
            .build(Root::builder().appender("stdout").build(lfilter))
            .unwrap();
        loghandle.set_config(config);
    }

    if let Some(url) = matches.get_one::<String>("url") {
        let mode = &URL_LUT[run_mode];
        conf.url = normalize_url(url, mode)?;
    }
    Ok(conf)
}
