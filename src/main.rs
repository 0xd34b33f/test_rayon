use humantime::format_duration;
use indicatif::{ProgressBar, ProgressStyle};
use rand::prelude::*;
use rayon::prelude::*;
use rayon::{spawn, ThreadPoolBuilder};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, Read};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender, SyncSender};
use std::thread::sleep;
use std::time::UNIX_EPOCH;
use std::time::{Duration, Instant};
use std::iter;
#[derive(Serialize, Debug, Clone)]
struct Response {
    result: String,
    hostname: String,
    process_time: String,
    status: bool,
}

fn construct_error<A>(
    hostname: &A,
    start_time: Instant,
    e: String,
    tx: &SyncSender<Response>,
) -> Response
    where
        A: Display + ToSocketAddrs,
{
    let response = Response {
        result: e,
        hostname: hostname.to_string(),
        process_time: format_duration(Instant::now() - start_time).to_string(),
        status: false,
    };
    match tx.send(response.clone()) {
        Ok(_) => (),
        Err(e) => eprintln!("Error sending response {}", e),
    }
    response
}



fn process_host_test<A>(hostname: A, command: &str, tx: SyncSender<Response>) -> Response
    where
        A: Display + ToSocketAddrs,
{
    let start_time = Instant::now();
    let mut rng = rand::rngs::OsRng;
    let stat: bool = rng.gen();
    let wait_time = rng.gen_range(2, 15);
    sleep(Duration::from_secs(wait_time));
    if !stat {
        return construct_error(
            &hostname,
            start_time,
            "Proizoshel trolling".to_string(),
            &tx,
        );
    }
    let end_time = Instant::now();
    let response = Response {
        hostname: hostname.to_string(),
        result: "test".to_string(),
        process_time: format_duration(end_time - start_time).to_string(),
        status: true,
    };
    match tx.send(response.clone()) {
        Ok(_) => (),
        Err(e) => eprintln!("Error sending data via channel"),
    };
    response
}

#[derive(Deserialize, Debug, Clone)]
struct OutputProps {
    save_to_file: bool,
    filename: Option<String>,
    pretty_format: bool,
    show_progress: bool,
    keep_incremental_data: Option<bool>,
}

#[derive(Deserialize, Debug, Clone)]
struct Config {
    threads: usize,
    output: OutputProps,
    command: String,
    timeout: u32,
}



fn main() {
    let queue_len = 10000;
    let hosts:Vec<String> = iter::repeat("lol".to_string()).take(queue_len).collect();
    let pool = ThreadPoolBuilder::new()
        .num_threads(2000)
        .build_global()
        .expect("failed creating pool");
    let (tx, rx): (SyncSender<Response>, Receiver<Response>) = mpsc::sync_channel(0);
    let datetime = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();
    let incremental_name = format!("incremental_{}.json", &datetime);
    let inc_for_closure = incremental_name.clone();
    spawn(move || incremental_save(rx,  queue_len as u64, incremental_name.as_str()));
    let result: Vec<Response> =
        hosts
            .par_iter()
            .map(|x| process_host_test(x, &"", tx.clone()))
            .collect();
}

fn progress_bar_creator(queue_len: u64) -> ProgressBar {
    let total_hosts_processed = ProgressBar::new(queue_len);
    let total_style = ProgressStyle::default_bar()
        .template("{eta} {wide_bar} Hosts processed: {pos}/{len} Speed: {per_sec} {msg}")
        .progress_chars("##-");
    total_hosts_processed.set_style(total_style);
    
    total_hosts_processed
}

fn incremental_save(rx: Receiver<Response>,  queue_len: u64, filename: &str) {
    let mut file = match File::create(Path::new(filename)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("incremental salving failed. : {}", e);
            return;
        }
    };
    dbg!("incremental_save started");
    let total = progress_bar_creator(queue_len);
    let mut ok = 0;
    let mut ko = 0;
    file.write("[\r\n".as_bytes())
        .expect("Writing for incremental saving failed");
    for _ in 0..queue_len + 1 {
        let received = match rx.recv() {
            Ok(a) => a,
            Err(e) => {
                eprintln!("incremental_save: {}", e);
                break;
            }
        };
        match received.status {
            true => ok += 1,
            false => ko += 1,
        };
        total.inc(1);
        total.set_message(&format!("OK: {}, Failed: {}", ok, ko));
        let mut data = serde_json::to_string_pretty(&received).unwrap();
        data += ",\n";
        file.write(data.as_bytes())
            .expect("Writing for incremental saving failed");
    }
    file.write("\n]".as_bytes())
        .expect("Writing for incremental saving failed");
}
