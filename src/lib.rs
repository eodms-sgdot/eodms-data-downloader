use ini::Ini;
use crate::queue::WorkQueue;
use reqwest::redirect;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::mpsc::channel;
use std::sync::mpsc::Sender;
use std::{thread, time};
use std::fs;
use std::str::FromStr;
use std::fs::File;
use std::io::{copy, Read};
use std::time::SystemTime;
use url::Url;

use crate::queue::new_syncflag;

pub mod queue;

type BoxResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug, PartialEq)]
pub enum RunMode {
    Dev,
    Prod,
}

impl FromStr for RunMode {
    type Err = Box<dyn Error>;
    fn from_str(input: &str) -> Result<RunMode, Self::Err> {
        match input {
            "Dev" => Ok(RunMode::Dev),
            "Prod" => Ok(RunMode::Prod),
            _ => Err("Invalid run mode, can only be Dev or Prod".into()),
        }
    }
}

pub struct ModeLu<'a> {
    url: &'a str,
}

pub const URL_LUT: [ModeLu; 2] = [
    ModeLu {
        url: "https://data.eodms-sgdot.nrcan-rncan.gc.ca",
    },
    ModeLu {
        url: "https://data.eodms-sgdot.nrcan-rncan.gc.ca",
    },
];

pub struct Conf {
    pub url: String,
    pub output_directory: String,
    pub recursive: bool,
    pub stripdirs: bool,
    pub num_threads: usize,
    pub creds: Option<Creds>,
    pub incrx: Option<regex::Regex>,
    pub queue: WorkQueue<File2Download>,
}

impl Default for Conf {
    fn default() -> Conf {
        Conf {
            url: String::new(),
            output_directory: String::new(),
            recursive: false,
            stripdirs: false,
            creds: get_creds(),
            num_threads: 4,
            incrx: None,
            queue: WorkQueue::new(),
        }
    }
}

pub struct File2Download {
    pub url: String,
    pub filename: String,
    pub creds: Option<Creds>,
    pub download: bool,
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
pub struct Creds {
    pub username: String,
    pub password: Option<String>,
}

fn get_creds() -> Option<Creds> {
    let mut homedir = get_homedir().unwrap_or_else(|e| panic!("{e}"));
    homedir.push(".eodms");
    homedir.push("config.ini");
    if homedir.as_path().exists() {
        log::debug!("Reading config.ini");
        let iniconf =
            Ini::load_from_file(homedir.as_os_str().to_str().ok_or("Error").ok()?).unwrap();
        let section = iniconf.section(Some("Credentials")).unwrap();
        let creds = Creds {
            username: section.get("username").unwrap().to_string(),
            password: Some(section.get("password").unwrap().to_string()),
        };
        log::info!("Loaded username {} from .eodms/config.ini", creds.username);
        return Some(creds);
    }
    log::warn!(
        "EODMS config file {} not found, connecting anonymously",
        homedir.as_path().display()
    );
    None
}

fn get_homedir() -> Result<PathBuf, Box<dyn Error>> {
    match dirs::home_dir() {
        Some(homedir) => Ok(homedir),
        None => Err("Home directory not found".into()),
    }
}

/// # Errors
///
/// TBD
/// # Panics
///
/// TBD
pub fn download_files(conf: &Conf) -> Result<(),Box<dyn Error>> {
    log::info!("Processing URL {}", conf.url);
    log::info!("Recursively: {}",conf.recursive);
    let (results_tx, results_rx) = channel();
    let (mut more_jobs_tx, more_jobs_rx) = new_syncflag(true);
    let mut threads = Vec::new();

    log::debug!("Spawning {} workers", conf.num_threads);
    for _thread_num in 0..conf.num_threads {
        let thread_queue = conf.queue.clone();
        let thread_results_tx: Sender<i32> = results_tx.clone();
        let thread_more_jobs_rx = more_jobs_rx.clone();
        let handle = thread::spawn(move || {
            let mut work_done = 0;
            while thread_more_jobs_rx.get().unwrap() {
                if let Some(work) = thread_queue.get_work() {
                    match download_file(&work) {
                        Ok(()) => work_done += 1,
                        Err(e) => {
                            log::error!("{e}");
                            break;
                        }
                    }
                    match thread_results_tx.send(work_done) {
                        Ok(()) => (),
                        Err(_) => {
                            break;
                        }
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
    match process_url(conf, &mut jobs_total) {
        Ok(()) => log::info!("Finished processing URL {}", conf.url),
        Err(e) => {
            log::error!("{e}");
            return Err(e);
        }
    }
    log::info!("{jobs_total} Files to Download");

    while jobs_total > 0 {
        match results_rx.recv() {
            Ok(_) => jobs_total -= 1,
            Err(e) => {
                log::error!("Receving result from thread");
                return Err(Box::new(e));
            }
        }
    }
    more_jobs_tx.set(false).unwrap();
    for handle in threads {
        handle.join().unwrap();
    }
    if jobs_total == 0 {
        log::info!("All files downloaded successfully");
    } else {
        log::error!("All files did not download successfully");
    }
    Ok(())
}

fn process_url(conf: &Conf, jobs_total: &mut u32) -> BoxResult<()> {
    let full_url = conf.url.clone() + "?_format=json";
    log::debug!("Full URL: {full_url}");
    #[allow(clippy::redundant_closure_for_method_calls)] // clippy suggestion does not work
    let custom = redirect::Policy::custom(|attempt| attempt.stop());

    let client = reqwest::blocking::Client::builder()
        .redirect(custom)
        .build()?;
    let response = client.head(&full_url);
    let response = match &conf.creds {
        Some(creds) => response.basic_auth(creds.username.clone(), creds.password.clone()),
        None => response,
    };
    let response = response.send()?;

    if response.status() != 200 {
        let err = format!("{}", response.status());
        log::error!("HTTP Error: {err}");
        return Err(err.as_str().into());
    }
    let entry_type = response.headers().get("entry-type");
    if let Some(value) = entry_type {
        let _result = match value.to_str() {
            Ok("Directory") => process_directory(conf, jobs_total),
            Ok("File") => {
                log::debug!("Original URL is a file");
                let fd = prepare_file(conf).unwrap();
                if fd.download {
                    log::debug!("pushing into queue");
                    conf.queue.add_work(fd);
                    *jobs_total += 1;
                }
                Ok(())
            }
            Ok(o) => return Err(format!("Invalid entry-type: {o}").into()),
            Err(_) => return Err("header error".into()),
        };
    } else {
        let err = "Header \"entry-type\" not found";
        log::error!("{err}");
        return Err(err.into());
    }
    Ok(())
}

fn process_directory(conf: &Conf, jobs_total: &mut u32) -> Result<(), Box<dyn Error>> {
    log::debug!("Process Directory - {}", conf.url);
    let full_url = conf.url.clone() + "?_format=json";
    let client = reqwest::blocking::Client::new();

    let response = client.get(full_url);
    let response = match &conf.creds {
        Some(creds) => response.basic_auth(creds.username.clone(), creds.password.clone()),
        None => response,
    };
    let response = response.send()?.json::<Vec<HashMap<String, String>>>()?;
    for entry in response {
        log::debug!("process_directory - Processing {}", entry["name"]);
        let mut new_url = conf.url.clone();
        new_url.push('/');
        new_url.push_str(entry["name"].as_str());
        log::debug!("NEW_URL: {new_url}");
        let new_conf = Conf {
            url: new_url,
            recursive: conf.recursive,
            stripdirs: conf.stripdirs,
            num_threads: conf.num_threads,
            creds: conf.creds.clone(),
            output_directory: conf.output_directory.clone(),
            incrx: conf.incrx.clone(),
            queue: conf.queue.clone(),
        };
        match entry["type"].as_str() {
            "file" => {
                let fd = prepare_file(&new_conf)?;
                if fd.download {
                    log::debug!("pushing into queue");
                    conf.queue.add_work(fd);
                    *jobs_total += 1;
                }
            },
            "directory" => {
                if conf.recursive {
                    match process_directory(&new_conf, jobs_total) {
                        Ok(()) => {}
                        Err(e) => return Err(e),
                    };
                } else {
                    return Ok(());
                }
            }
            &_ => return Err("Invalid entry type".into()),
        }
    }
    Ok(())
}

fn prepare_file(conf: &Conf) -> Result<File2Download, Box<dyn Error>> {
    let parsed_url = Url::parse(conf.url.as_str())?;
    log::debug!("{}", parsed_url.path());
    let mut pathsegs = parsed_url
        .path_segments()
        .map(std::iter::Iterator::collect::<Vec<_>>)
        .expect("Unable to parse path segments");
    let filename = pathsegs.pop().expect("URL Parsing Error");
    if conf.stripdirs {
        pathsegs.clear();
    }
    let directory = pathsegs.join("/");

    log::debug!("Filename: {filename}");
    log::debug!("Directory: {directory}");
    let fqdir = if directory.is_empty() {
        conf.output_directory.clone()
    } else {
        format!("{}/{}", conf.output_directory, directory)
    };
    log::debug!("FQDIR: {fqdir}");
    let dirpath = Path::new(&fqdir);
    let fname = fqdir.clone() + "/" + filename;
    let mut fd = File2Download {
        filename: fname.clone(),
        url: conf.url.clone(),
        creds: conf.creds.clone(),
        download: false,
    };
    if let Some(incrx) = &conf.incrx {
        log::debug!("Checking {} against {:?}", fname, conf.incrx);
        if incrx.is_match(fname.as_str()) {
            log::debug!("Matched");
        } else {
            log::debug!("Not matched");
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
        File::create(fd.filename.clone())
            .unwrap_or_else(|e| panic!("Error {} creating file {}", e, fd.filename))
    };

    let client = reqwest::blocking::Client::builder().timeout(None).build()?;

    let response = client.get(fd.url.clone());
    let response = match &fd.creds {
        Some(creds) => response.basic_auth(creds.username.clone(), creds.password.clone()),
        None => response,
    };
    let mut response = response.send()?;
    let map = response.headers().clone();
    let len = map
        .get("content-length")
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

    log::info!(
        "File {} - Downloaded in {delta:.prec$} seconds at {rate:.prec$} MiBps",
        fd.filename,
        prec = 2,
        delta = delta,
        rate = rate
    );
    Ok(())
}

/// # Errors
///
/// Will return an error given an invalid url
pub fn normalize_url(url: &str, mode: &ModeLu) -> Result<String,Box<dyn Error>> {
    let base_url_len = mode.url.len();
    let trimmed_url = url.trim_end_matches('/');

    let url_len = trimmed_url.to_string().len();
    if url_len < base_url_len + 1 {
        return Err(format!("Invalid URL {trimmed_url}").into());
    }
    let slice = &url[..base_url_len];
    if slice == mode.url {
        return Ok(trimmed_url.to_string());
    }
    Err(format!("Invalid URL {trimmed_url}").into())
}
