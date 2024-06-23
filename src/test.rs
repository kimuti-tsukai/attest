use std::env::current_dir;
use std::fs::File;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::num::IntErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Child, Command as StdCommand, Output, Stdio};
use std::time::{Duration, Instant};

use crate::utils::{
    file_read_to_string, items_toml, make_client, request, to_html, link_from_copy,
    CREATE_ERR, WRITE_ERR,
};

use anyhow::Result;

use reqwest::Client;

use rustc_hash::FxHasher;

use tokio::process::Command;
use tokio::time;

use toml::{map::Map, Value};

use serde::{Deserialize, Serialize};

use scraper::{Html, Selector};

use regex::Regex;


// Function to test
pub async fn test(url: Option<String>, example_num: Vec<usize>) -> Result<Option<Vec<Res>>> {
    let (examples, time_limit): (Vec<IO>, u128);

    if url.is_none() || is_same_link(&url.clone().unwrap()) {
        examples = examples_from_cache();

        time_limit = time_limit_from_cache();
    } else {
        let url: String = url.unwrap();

        let c: Client = make_client();

        let text: String = request(&c, &url).await?;

        let html: Html = to_html(text);

        examples = assort(&select_samples(&html));

        time_limit = get_time_limit(&html);

        save_cache(&url, time_limit, &examples);
    }
    let setting_toml: Map<String, Value> = items_toml("./test.toml");

    let results: Option<Vec<Res>> = tester(&examples, &setting_toml, time_limit, example_num).await;

    Ok(results)
}

// Check if the link is same
fn is_same_link(url: &str) -> bool {
    url == link_from_copy()
}

// Get examples from cache if the link is same
fn examples_from_cache() -> Vec<IO> {
    let text: String = file_read_to_string("./.attest/examples.json");

    serde_json::from_str(&text.trim()).unwrap()
}

// Get time limit from cache if the link is same
fn time_limit_from_cache() -> u128 {
    let time: String = file_read_to_string("./.attest/time_limit.txt");

    time.parse().unwrap()
}

// Save cache if the link is different
fn save_cache(url: &str, time_limit: u128, examples: &Vec<IO>) {
    let mut l: File = File::create("./.attest/url.txt").expect(CREATE_ERR);
    write!(&mut l, "{}", url).expect(WRITE_ERR);

    let mut t: File = File::create("./.attest/time_limit.txt").expect(CREATE_ERR);
    write!(&mut t, "{}", time_limit).expect(WRITE_ERR);

    let mut e: File = File::create("./.attest/examples.json").expect(CREATE_ERR);
    write!(&mut e, "{}", serde_json::to_string(examples).unwrap()).expect(WRITE_ERR);
}

// Select examples from Html
fn select_samples(html: &Html) -> Vec<String> {
    let selector: Selector = Selector::parse(r#"span[class="lang-ja"] h3 + pre"#).unwrap();

    let mut samples: Vec<String> = Vec::new();

    for i in html.select(&selector) {
        let v: Vec<&str> = i.text().collect();
        samples.push(v.join(""));
    }

    samples
}

// Input and Output of example
#[derive(Serialize, Deserialize, Debug, Clone)]
struct IO {
    pub input: String,
    pub output: String,
}

impl IO {
    // Constructor
    pub fn new(input: String, output: String) -> Self {
        IO { input, output }
    }
}

// Packing inputs and outputs to `IO`
fn assort(v: &[String]) -> Vec<IO> {
    v.chunks(2)
        .map(|l: &[String]| IO::new(l[0].clone(), l[1].clone()))
        .collect()
}

// Get time limit
fn get_time_limit(html: &Html) -> u128 {
    let selector: Selector = Selector::parse(r#"div[class="col-sm-12"] > p"#).unwrap();

    let t: &str = html
        .select(&selector)
        .next()
        .unwrap()
        .text()
        .next()
        .unwrap();

    let re1: Regex = Regex::new("Time Limit: (.+) sec").unwrap();
    let re2: Regex = Regex::new("Time Limit: (.+) msec").unwrap();

    if let Some(s) = re1.captures(t) {
        (s.get(1).unwrap().as_str().parse::<f64>().unwrap() * 1000.) as u128
    } else {
        re2.captures(t)
            .unwrap()
            .get(1)
            .unwrap()
            .as_str()
            .parse::<u128>()
            .unwrap()
    }
}

// Check if the code is same
fn is_same_code(setting_toml: &Map<String, Value>) -> Option<bool> {
    let file_path: &str = setting_toml
        .get("file_path")?
        .as_str()
        .expect(r#"the "file_path" value must be string"#);

    if !Path::new(file_path).is_file() {
        return None;
    }

    let now: String = file_read_to_string(file_path);

    let mut now_hasher: FxHasher = FxHasher::default();

    now.hash(&mut now_hasher);

    let now_hash: u64 = now_hasher.finish();

    let before: String = file_read_to_string("./.attest/before.txt");

    let before_hash: Option<u64> = match before.parse() {
        Ok(x) => Some(x),
        Err(e) => match e.kind() {
            IntErrorKind::Empty => None,
            _ => return None,
        },
    };

    if Some(now_hash) == before_hash {
        Some(true)
    } else {
        let mut f: File = File::create("./.attest/before.txt").expect(CREATE_ERR);
        write!(&mut f, "{}", now_hash).expect(WRITE_ERR);
        Some(false)
    }
}

fn is_same_setting(setting_toml: &Map<String, Value>) -> Result<bool> {
    let before_settiing: Map<String, Value> = items_toml("./.attest/cache.toml");

    if setting_toml == &before_settiing {
        Ok(true)
    } else {
        let mut f: File = File::create("./.attest/cache.toml").expect(CREATE_ERR);
        writeln!(&mut f, "{}", setting_toml).expect(WRITE_ERR);

        Ok(false)
    }
}

fn build(setting_toml: &Map<String, Value>, dir: &PathBuf) -> Option<Output> {
    if let (Some(c), Some(code), Ok(setting)) = (
        setting_toml.get("build"),
        is_same_code(setting_toml),
        is_same_setting(setting_toml),
    ) {
        if code && setting {
            return None;
        }

        let build_commands: Vec<&str> = c
            .as_array()
            .expect(r#""build" value must be array"#)
            .iter()
            .map(|v: &Value| {
                v.as_str()
                    .expect(r#"items of "build" value must be string"#)
            })
            .collect();

        let command: &str = build_commands.first()?.to_owned();

        let args: &[&str] = if build_commands.len() > 1 {
            &build_commands[1..]
        } else {
            &[]
        };

        Some(
            StdCommand::new(command)
                .args(args)
                .current_dir(dir)
                .output()
                .expect("Something went wrong when building program"),
        )
    } else {
        None
    }
}

fn build_wrap(
    setting_toml: &Map<String, Value>,
    dir: &PathBuf,
    results: &mut Vec<Res>,
) -> Result<(), ()> {
    if let Some(output) = build(setting_toml, dir) {
        if output.status.code().unwrap() != 0 {
            println!("\x1b[33mCE\x1b[m\n");
            println!("stderr:\n{}", std::str::from_utf8(&output.stderr).unwrap());
            results.push(Res::CE);

            return Err(());
        }
    }

    Ok(())
}

fn get_commands(setting_toml: &Map<String, Value>) -> Vec<String> {
    setting_toml
        .get("run")
        .expect(r#""test.toml" must have "run" value"#)
        .as_array()
        .expect(r#""run" value must be array"#)
        .iter()
        .map(|v: &Value| {
            v.as_str()
                .expect(r#"items of "run" value must be string"#)
                .to_string()
        })
        .collect()
}

fn get_test_command(setting_toml: &Map<String, Value>) -> Option<Vec<String>> {
    Some(
        setting_toml
            .get("test")?
            .as_array()
            .expect(r#""test" value must be array"#)
            .iter()
            .map(|v: &Value| {
                v.as_str()
                    .expect(r#"items of "run" value must be string"#)
                    .to_string()
            })
            .collect(),
    )
}

fn spawn_command(
    io: &IO,
    dir: &PathBuf,
    execute_command: &str,
    args: &[String],
) -> impl Future<Output = Result<Output, std::io::Error>> {
    let pipe: Child = StdCommand::new("echo")
        .arg(&io.input)
        .current_dir(dir)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    Command::new(execute_command)
        .args(args)
        .stdin(Stdio::from(pipe.stdout.unwrap()))
        .current_dir(dir)
        .output()
}

async fn tester(
    examples: &[IO],
    setting_toml: &Map<String, Value>,
    time_limit: u128,
    example_num: Vec<usize>,
) -> Option<Vec<Res>> {
    let dir: PathBuf = current_dir().unwrap();

    let mut results: Vec<Res> = Vec::new();

    if build_wrap(setting_toml, &dir, &mut results).is_err() {
        return None;
    }

    let commands: Vec<String> = get_commands(setting_toml);

    let execute_command: &str = commands
        .first()
        .expect(r#""command" value is not satisfied"#);

    let args: &[String] = if commands.len() > 1 {
        &commands[1..]
    } else {
        &[]
    };

    let test_commands: Option<Vec<String>> = get_test_command(setting_toml);

    for (index, io) in examples.iter().enumerate() {
        if !example_num.is_empty() && !example_num.contains(&(index + 1)) {
            continue;
        }

        println!("example{}", index + 1);

        let output = spawn_command(io, &dir, execute_command, args);

        let start: Instant = Instant::now();

        let output: Output =
            match time::timeout(Duration::from_millis(time_limit as u64), output).await {
                Ok(v) => {
                    if let Ok(v) = v {
                        v
                    } else {
                        continue;
                    }
                }
                Err(_) => {
                    let time: u128 = start.elapsed().as_millis();

                    println!("\x1b[33mTLE\x1b[m");

                    println!();

                    println!("input:\n{}", io.input);
                    println!("expect output:\n{}", io.output);

                    println!("time: {}", time);

                    results.push(Res::TLE);
                    continue;
                }
            };

        let time: u128 = start.elapsed().as_millis();

        results.push(check(output, time, io, &test_commands, &dir));
    }

    for (i, r) in results.iter().enumerate() {
        println!(
            "example{}: {}",
            i + 1,
            match r {
                Res::AC => "\x1b[32mAC\x1b[m",
                Res::WA => "\x1b[33mWA\x1b[m",
                Res::RE => "\x1b[33mRE\x1b[m\n",
                Res::TLE => "\x1b[33mTLE\x1b[m",
                _ => "",
            }
        );
    }

    Some(results)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Res {
    AC,
    WA,
    CE,
    RE,
    #[allow(clippy::upper_case_acronyms)]
    TLE,
}

fn spawn_test_command(
    test_command: &Option<Vec<String>>,
    result: &str,
    io: &IO,
    dir: &PathBuf,
) -> bool {
    let command: &Vec<String> = test_command.as_ref().unwrap();

    let pipe: Child = StdCommand::new("echo")
        .args([result, &io.output])
        .current_dir(dir)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let test_output: Output = StdCommand::new(command.first().unwrap())
        .args([result, &io.output])
        .stdin(Stdio::from(pipe.stdout.unwrap()))
        .current_dir(dir)
        .output()
        .unwrap();

    let judge: &str = std::str::from_utf8(&test_output.stdout).unwrap_or("");

    match judge {
        "true" => true,
        "false" => false,
        _ => panic!("The test command failed"),
    }
}

fn check(
    output: Output,
    time: u128,
    io: &IO,
    test_command: &Option<Vec<String>>,
    dir: &PathBuf,
) -> Res {
    let result: &str = std::str::from_utf8(&output.stdout).unwrap_or("");

    let return_value: Res = if output.status.code() == Some(0) {
        let condition: bool =
            if test_command.is_some() && !test_command.as_ref().unwrap().is_empty() {
                spawn_test_command(test_command, result, io, dir)
            } else {
                result == io.output
            };

        if condition {
            println!("\x1b[32mAC\x1b[m");
            println!("input:\n{}", io.input);
            Res::AC
        } else {
            println!("\x1b[33mWA\x1b[m");

            println!();

            println!("input:\n{}", io.input);
            println!("excepted output:\n{}", io.output);
            Res::WA
        }
    } else {
        println!("\x1b[33mRE\x1b[m\n");
        println!("input:\n{}", io.input);
        Res::RE
    };

    println!("output:\n{}", result);

    println!("stderr:\n{}", std::str::from_utf8(&output.stderr).unwrap());

    println!("time: {}", time);

    return_value
}
