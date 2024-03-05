// Copyright ⓒ  His Majesty the King in Right of Canada, as
// represented by the Minister of Natural Resources, 2022
// Licensed under the MIT license
// (see LICENSE or <http://opensource.org/licenses/MIT>) All files in the project carrying such
// notice may not be copied, modified, or distributed except according to those terms.

#[cfg(feature = "unlimited")]
#[macro_use]
extern crate simple_error;
#[macro_use]
extern crate env_var;
extern crate dirs;
use std::str::FromStr;
use std::process;
use std::error::Error;
use std::path::Path;
use std::fs;
use std::io::{copy,Read};
use std::fs::File;
use std::path::PathBuf;
use std::time::{SystemTime};
use regex::Regex;
use reqwest::redirect;
use std::collections::HashMap;
use clap::{Arg, ArgAction, Command};
use url::Url;
use log::{info,debug,error,LevelFilter,warn};
use log4rs::append::console::ConsoleAppender;
use log4rs::encode::pattern::PatternEncoder;
use log4rs::config::{Appender, Root};
use log4rs::Config;
extern crate ini;
use ini::Ini;
use std::sync::mpsc::channel;
use std::sync::mpsc::Sender;
use std::{thread, time};
mod queue;
use crate::queue::WorkQueue;
use crate::queue::new_syncflag;

#[derive(Debug, PartialEq)]
enum RunMode {
	Dev,
	Prod,
}

impl FromStr for RunMode {
	type Err = Box<dyn Error>;
	fn from_str(input: &str) -> Result<RunMode, Self::Err> {
		match input {
			"Dev"    => Ok(RunMode::Dev),
			"Prod"   => Ok(RunMode::Prod),
			_        => Err("Invalid run mode, can only be Dev or Prod".into()),
		}
	}
}

struct ModeLu<'a> {
	url: &'a str,
}
const URL_LUT: [ModeLu;2] = [
	ModeLu  { url: "https://data.eodms-sgdot.nrcan-rncan.gc.ca" },
	ModeLu  { url: "https://data.eodms-sgdot.nrcan-rncan.gc.ca" },
];

type BoxResult<T> = Result<T,Box<dyn Error>>;

struct Conf {
	url: String,
	output_directory: String,
	recursive: bool,
	stripdirs: bool,
	num_threads: usize,
	creds: Option<Creds>,
	incrx: Option<regex::Regex>,
	queue: WorkQueue<File2Download>,
}

impl Default for Conf {
	fn default() -> Conf {
		Conf {
			url: "".to_string(),
			output_directory:"".to_string(),
			recursive: false,
			stripdirs: false,
			creds: get_creds(),
			num_threads: 4,
			incrx: None,
			queue: WorkQueue::new(),
		}
	}
}

struct File2Download {
	url: String,
	filename: String,
	creds: Option<Creds>,
	download: bool,
}

impl Clone for File2Download {
	fn clone(&self) -> Self {
		File2Download {
			url: self.url.clone(),
			filename: self.filename.clone(),
			creds: self.creds.clone(),
			download: self.download,
		}
	}
}

#[derive(Clone)]
struct Creds {
	username: String,
	password: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
	let conf = process_args()?;
	info!("Processing URL {}",conf.url);
	let (results_tx, results_rx) = channel();
	let (mut more_jobs_tx, more_jobs_rx) = new_syncflag(true);
	let mut threads = Vec::new();

	debug!("Spawning {} workers", conf.num_threads);
	for _thread_num in 0..conf.num_threads {
		let thread_queue = conf.queue.clone();
		let thread_results_tx: Sender<i32> = results_tx.clone();
		let thread_more_jobs_rx = more_jobs_rx.clone();
		let handle = thread::spawn(move || {
			let mut work_done = 0;
			while thread_more_jobs_rx.get().unwrap() {
				if let Some(work) = thread_queue.get_work() {
					match download_file(&work) {
						Ok(()) => { work_done += 1 },
						Err(e) => { error!("{}",e);break; },
					};
					match thread_results_tx.send(work_done) {
						Ok(_) => (),
						Err(_) => { break; },
					}
				}
				let ten_millis = time::Duration::from_millis(10);
				thread::sleep(ten_millis);
				std::thread::yield_now();
			}
		});
		threads.push(handle);
	}

	let mut jobs_total: u32 = 0;
	match process_url(&conf,&mut jobs_total) {
			Ok(()) => info!("Finished processing URL {}",conf.url),
			Err(e) => {
				error!("{}", e);
				return Err(e);
			},
	};
	info!("{} Files to Download",jobs_total);

	while jobs_total > 0 {
		match results_rx.recv() {
			Ok(_) => { jobs_total -= 1 },
			Err(e) => {
				error!("Receving result from thread");
				return Err(Box::new(e));
			}
		}
	}
	more_jobs_tx.set(false).unwrap();
	for handle in threads {
		handle.join().unwrap();
	}
	if jobs_total == 0 {
		info!("All files downloaded successfully");
	} else {
		error!("All files did not download successfully");
	}
	Ok(())
}

fn process_args() -> Result<Conf, Box<dyn Error>> {
	let env_mode = env_var!(optional "MODE", default: "Prod");
	let run_mode = RunMode::from_str(env_mode.as_str())? as usize;
	let stdout = ConsoleAppender::builder()
		.encoder(Box::new(PatternEncoder::new("{d(%Y-%m-%d %T%.3f)(utc)} [{l}] - {m}{n}")))
		.build();
	let filter = match run_mode {
		0 => LevelFilter::Debug,
		1 => LevelFilter::Info,
		_ => LevelFilter::Info,
	};
	let config = Config::builder()
		.appender(Appender::builder().build("stdout", Box::new(stdout)))
		.build(Root::builder().appender("stdout").build(filter))
		.unwrap();
	let loghandle = log4rs::init_config(config).unwrap();
	let mode = &URL_LUT[run_mode];
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
						.num_args(1)
				)
				.arg(
					Arg::new("out")
						.short('o')
						.long("outdir")
						.help("output directory")
						.action(ArgAction::Set)
						.required(true)
						.num_args(1)
				)
				.arg(
					Arg::new("rec")
						.short('r')
						.long("rec")
						.help("recursive")
						.required(false)
						.num_args(0)
						.value_parser(clap::builder::BoolishValueParser::new())
				)
				.arg(
					Arg::new("inc")
						.short('i')
						.long("include")
						.help("include regex")
						.action(ArgAction::Set)
				)
				.arg(
					Arg::new("threads")
						.short('t')
						.long("threads")
						.value_parser(clap::value_parser!(u8))
						.help("number of download threads")
						.action(ArgAction::Set)
				)
				.arg(
					Arg::new("loglevel")
						.short('l')
						.long("loglevel")
						.help("logging level off, error, info, debug, trace")
						.action(ArgAction::Set)
				)
				.arg(
					Arg::new("stripdirs")
						.short('s')
						.long("stripdirs")
						.help("strip the leading directories")
						.num_args(0)
						.value_parser(clap::builder::BoolishValueParser::new())
				)
		.get_matches();
	let mut conf = Conf::default();
	if let Some(out) = matches.get_one::<String>("out") {
		let path = Path::new(out);
		if path.exists() {
			if !path.is_dir() {
				error!("{:?} exists and is not a directory",path);
				let err = format!("{:?} exists and is not a directory",path);
				return Err(err.as_str().into());
			}
		} else {
			fs::create_dir(out)?;
		}
		conf.output_directory = out.to_string();
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
			"off"   => LevelFilter::Off,
			"error" => LevelFilter::Error,
			"warn"  => LevelFilter::Warn,
			"info"  => LevelFilter::Info,
			"debug" => LevelFilter::Debug,
			"trace" => LevelFilter::Trace,
			&_      => return Err("Invalid loglevel, needs to be one of: off,error,warn,info,debug or trace".into()),
		};
		let stdout = ConsoleAppender::builder()
			.encoder(Box::new(PatternEncoder::new("{d(%Y-%m-%d %T%.3f)(utc)} [{l}] - {m}{n}")))
			.build();
		let config = Config::builder()
			.appender(Appender::builder().build("stdout", Box::new(stdout)))
			.build(Root::builder().appender("stdout").build(lfilter))
			.unwrap();
		loghandle.set_config(config);
	}

	if let Some(url) = matches.get_one::<String>("url") {
		let base_url_len = mode.url.len();
		let trimmed_url = url.as_str().trim_end_matches('/');

		let url_len = trimmed_url.to_string().len();
		if url_len < base_url_len+1 {
			error!("Invalid URL {}",trimmed_url);
			process::exit(1);
		}
		let slice = &url[..base_url_len];
		if slice == mode.url {
			conf.url = trimmed_url.to_string();
		} else {
			error!("Invalid URL {}",trimmed_url);
			process::exit(1);
		}
	}
	Ok(conf)
}

fn process_url(conf: &Conf, jobs_total: &mut u32) -> BoxResult<()> {
	let full_url = conf.url.clone() + "?_format=json";
	debug!("Full URL: {}",full_url);
	let custom = redirect::Policy::custom(|attempt| attempt.stop());

	let client = reqwest::blocking::Client::builder()
		.redirect(custom)
		.build()?;
	let response = client.head(&full_url);
	let response = match &conf.creds {
		Some(creds) => response.basic_auth(creds.username.clone(), creds.password.clone()),
		None        => response,
	};
	let response = response
		.send()?;

	if response.status() != 200 {
		let err = format!("{}",response.status());
		error!("HTTP Error: {}",err);
		return Err(err.as_str().into());
	}
	let entry_type = response.headers().get("entry-type");
	match entry_type {
		Some(value) => {
			let _result = match value.to_str() 	{
				Ok("Directory") => process_directory(conf,jobs_total),
				Ok("File")      => {
					debug!("Original URL is a file");
					let fd = prepare_file(conf).unwrap();
					if fd.download  {
						debug!("pushing into queue");
						conf.queue.add_work(fd);
						*jobs_total += 1;
					}
					Ok(())
				}
				Ok(o)  => return Err(format!("Invalid entry-type: {}",o).into()),
				Err(_) => return Err("header error".into()),
			};
		},
		None => {
			let err = "Header \"entry-type\" not found";
			error!("{}",err);
			return Err(err.into());
		}
	}
	Ok(())
}

fn process_directory(conf: &Conf, jobs_total: &mut u32) -> Result<(), Box<dyn Error>> {
	debug!("Process Directory - {}",conf.url);
	let full_url = conf.url.clone() + "?_format=json";
	let client = reqwest::blocking::Client::new();

	let response = client.get(full_url);
	let response = match &conf.creds {
		Some(creds) => response.basic_auth(creds.username.clone(), creds.password.clone()),
		None        => response,
	};
	let response = response
		.send()?
		.json::<Vec<HashMap<String, String>>>()?;
	for entry in response {
		debug!("process_directory - Processing {}",entry["name"]);
		let mut new_url = conf.url.clone();
		new_url.push('/');
		new_url.push_str(entry["name"].as_str());
		debug!("NEW_URL: {}",new_url);
		let new_conf = Conf {
			url: new_url,
			recursive: conf.recursive,
			stripdirs: conf.stripdirs,
			num_threads: conf.num_threads,
			creds: conf.creds.clone(),
			output_directory: conf.output_directory.to_string(),
			incrx: conf.incrx.clone(),
			queue: conf.queue.clone(),
		};
		match entry["type"].as_str() {
			"file"      =>
				match prepare_file(&new_conf) {
					Ok(fd) => {
						if fd.download {
							debug!("pushing into queue");
							conf.queue.add_work(fd);
							*jobs_total += 1;
						}
					},
					Err(e) => return Err(e),
				},
			"directory" => {
				if conf.recursive {
					match process_directory(&new_conf,jobs_total) {
						Ok(()) => {},
						Err(e) => return Err(e),
					};
				} else {
					return Ok(());
				}
			},
			&_ => return Err("Invalid entry type".into()),
		}
	}
	Ok(())
}

fn prepare_file(conf: &Conf) -> Result<File2Download, Box<dyn Error>> {
	let parsed_url = Url::parse(conf.url.as_str())?;
	debug!("{}",parsed_url.path());
	let mut pathsegs = parsed_url.path_segments()
		.map(|c| c.collect::<Vec<_>>()).
		expect("Unable to parse path segments");
	let filename = pathsegs.pop().expect("URL Parsing Error");
	if conf.stripdirs {
		pathsegs.clear();
	}
	let directory = pathsegs.join("/");

	debug!("Filename: {}",filename);
	debug!("Directory: {}",directory);
	let fqdir = format!("{}/{}",conf.output_directory,directory);
	let dirpath = Path::new(&fqdir);
	let fname = fqdir.clone() + "/" + filename;
	let mut fd = File2Download {
		filename: fname.clone(),
		url: conf.url.clone(),
		creds: conf.creds.clone(),
		download: false
	};
	if conf.incrx.is_some() {
		debug!("Checking {} against {:?}",fname,conf.incrx);
		if conf.incrx.as_ref().unwrap().is_match(fname.as_str()) {
			debug!("Matched");
		} else {
			debug!("Not matched");
			return Ok(fd);
		}
	}
	fd.download = true;
	if !dirpath.exists() {
		fs::create_dir_all(dirpath)?;
	}
	Ok(fd)
}

fn download_file(fd: &File2Download) -> Result<(), Box<dyn Error>> {
	let mut dest = {
		File::create(fd.filename.clone()).unwrap_or_else(|e| panic!("Error {} creating file {}",e,fd.filename))
	};

	let client = reqwest::blocking::Client::builder()
	.timeout(None).build()?;

	let response = client.get(fd.url.clone());
	let response = match &fd.creds {
		Some(creds) => response.basic_auth(creds.username.clone(), creds.password.clone()),
		None        => response,
	};
	let mut response = response.send()?;
	let map = response.headers().clone();
	let len = map.get("content-length")
		.expect("Missing content-length header")
		.to_str()?
		.parse::<f64>()?;

	let start = SystemTime::now();

	let mut buffer = [0; 16384];
	loop {
		let n = response.read(&mut buffer[..])?;
		if n == 0 {
			break;
		}
		let mut slice = &buffer[0..n];
		copy(&mut slice, &mut dest)?;
	}
	let stop = SystemTime::now();
	let delta = stop.duration_since(start)?.as_secs_f64();

	let rate = match len / delta / 1024.0 / 1024.0 {
		x if x.is_infinite() => len / 1024.0 / 1024.0,
		x => x,
	};

	info!("File {} - Downloaded in {delta:.prec$} seconds at {rate:.prec$} MiBps",fd.filename,prec=2,delta=delta,rate=rate);
	Ok(())
}

fn get_homedir() -> Result<PathBuf, Box<dyn Error>> {
	match dirs::home_dir() {
		Some(homedir) => Ok(homedir),
		None          => Err("Home directory not found".into()),
	}
}

fn get_creds() -> Option<Creds> {
	let mut homedir = get_homedir().unwrap_or_else(|e| panic!("{} - ",e));
	homedir.push(".eodms");
	homedir.push("config.ini");
	if homedir.as_path().exists() {
		debug!("Reading config.ini");
		let iniconf = Ini::load_from_file(homedir.as_os_str().to_str().ok_or("Error").ok()?).unwrap();
		let section = iniconf.section(Some("Credentials")).unwrap();
		let creds = Creds {
			username : section.get("username").unwrap().to_string(),
			password : Some(section.get("password").unwrap().to_string()),
		};
		info!("Loaded username {} from .eodms/config.ini",creds.username);
		return Some(creds);
	} else {
		warn!("EODMS config file {} not found, connecting anonymously",homedir.as_path().display());
	}
	None
}
