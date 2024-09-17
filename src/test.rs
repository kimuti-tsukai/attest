use std::{
    collections::HashMap,
    env::current_dir,
    fmt::Write as _,
    fs::{self, File},
    future::Future,
    hash::{Hash, Hasher},
    io::Write,
    num::IntErrorKind,
    path::{Path, PathBuf},
    process::{Command as StdCommand, Output, Stdio},
    time::{Duration, Instant},
};

use crate::utils::{
    file_read_to_string, items_toml, link_from_copy, make_client, request, to_html, Marker,
    CREATE_ERR, WRITE_ERR,
};

use anyhow::{bail, Context, Result};

use reqwest::Client;

use rustc_hash::FxHasher;

use tokio::{
    io::AsyncWriteExt,
    process::{Child, Command},
    time,
};

use toml::{map::Map, Value};

use serde::{Deserialize, Serialize};

use scraper::{Html, Selector};

use regex::Regex;

use proconio_derive::fastout;

// Function to test
pub async fn test(
    url: Option<String>,
    example_num: Vec<usize>,
    p_build: bool,
) -> Result<Option<Vec<Option<Res>>>> {
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
    let setting_toml: Map<String, Value> = items_toml("./attest.toml");

    let results: Option<Vec<Option<Res>>> =
        tester(&examples, &setting_toml, time_limit, example_num, p_build).await;

    Ok(results)
}

// Check if the link is same
fn is_same_link(url: &str) -> bool {
    url == link_from_copy()
}

// Get examples from cache if the link is same
fn examples_from_cache() -> Vec<IO> {
    let text: String = file_read_to_string("./.attest/examples.json");

    serde_json::from_str(text.trim()).unwrap()
}

// Get time limit from cache if the link is same
fn time_limit_from_cache() -> u128 {
    let time: String = file_read_to_string("./.attest/time_limit.txt");

    time.trim().parse().unwrap()
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
            .unwrap_or_else(|| panic!("{} Please input the question page.", Marker::Minus))
            .as_str()
            .parse::<u128>()
            .unwrap()
    }
}

// Check if the code is same
fn is_same_code(setting_toml: &Map<String, Value>) -> Option<bool> {
    let file_path: &str = setting_toml.get("file_path")?.as_str().unwrap_or_else(|| {
        panic!(
            "{}",
            Marker::minus(r#"the "file_path" value must be string"#)
        )
    });

    if !Path::new(file_path).is_file() {
        return None;
    }

    let now: String = file_read_to_string(file_path);

    let mut now_hasher: FxHasher = FxHasher::default();

    now.hash(&mut now_hasher);

    let now_hash: u64 = now_hasher.finish();

    let before: String = file_read_to_string("./.attest/before.txt");

    let before_hash: Option<u64> = match before.trim().parse() {
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

fn is_sama_other_files(setting_toml: &Map<String, Value>) -> bool {
    let Some(file_paths) = setting_toml.get("deps_files") else {
        return true;
    };

    let files = if let Some(l) = file_paths.as_array() {
        l.iter().map(|v: &Value| {
            v.as_str().unwrap_or_else(|| {
                panic!(
                    "{} The values of deps_files array have to be str",
                    Marker::Minus
                )
            })
        })
    } else {
        eprintln!("{} The deps_files value has to be array", Marker::Minus);
        return true;
    };

    let file_hashes = files.map(|p: &str| {
        let f: String = file_read_to_string(p);
        let mut hasher: FxHasher = FxHasher::default();
        f.hash(&mut hasher);
        (p, hasher.finish())
    });

    let caches_t: String =
        fs::read_to_string("./.attest/deps_caches.json").unwrap_or("{}".to_string());

    let mut caches: HashMap<&str, u64> = serde_json::from_str(&caches_t).unwrap_or_default();

    let mut new_cache: HashMap<&str, u64> = HashMap::new();

    let mut is_same: bool = true;

    for (key, hash) in file_hashes {
        if !caches.get(&key).is_some_and(|v: &u64| *v == hash) {
            is_same = false;
        }
        caches.remove(&key);
        new_cache.insert(key, hash);
    }

    is_same = is_same && caches.is_empty();

    if !is_same {
        let mut f: File = File::create("./.attest/deps_caches.json").expect(CREATE_ERR);
        let s: String = serde_json::to_string(&new_cache).unwrap();
        write!(f, "{}", s).expect(WRITE_ERR);
    }

    is_same
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

fn build<T: AsRef<Path>>(
    setting_toml: &Map<String, Value>,
    dir: T,
    p_build: bool,
) -> Option<Output> {
    if let Some(c) = setting_toml.get("build") {
        if !p_build
            && is_same_code(setting_toml).unwrap_or(true)
            && is_same_setting(setting_toml).unwrap_or(true)
            && is_sama_other_files(setting_toml)
        {
            return None;
        }

        let build_commands: Vec<&str> = c
            .as_array()
            .unwrap_or_else(|| panic!("{}", Marker::minus(r#""build" value must be array"#)))
            .iter()
            .map(|v: &Value| {
                v.as_str().unwrap_or_else(|| {
                    panic!(
                        "{}",
                        Marker::minus(r#"items of "build" value must be string"#)
                    )
                })
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
                .unwrap_or_else(|_| {
                    panic!(
                        "{}",
                        Marker::minus("Something went wrong when building program")
                    )
                }),
        )
    } else {
        None
    }
}

fn build_wrap<T: AsRef<Path>>(
    setting_toml: &Map<String, Value>,
    dir: T,
    results: &mut Vec<Option<Res>>,
    p_build: bool,
) -> Result<(), String> {
    let mut buf: String = String::new();
    if let Some(output) = build(setting_toml, dir, p_build) {
        if output.status.code().unwrap() != 0 {
            writeln!(buf, "{} \x1b[33mCE\x1b[m\n", Marker::Minus).unwrap();
            writeln!(
                buf,
                "{} stderr:\n{}",
                Marker::X,
                std::str::from_utf8(&output.stderr).unwrap()
            )
            .unwrap();
            results.push(Some(Res::CE));

            return Err(buf);
        }
    }

    Ok(())
}

fn get_string_list(key: &str, toml: &Map<String, Value>) -> Option<Vec<String>> {
    Some(
        toml.get(key)?
            .as_array()
            .unwrap_or_else(|| panic!(r#""{}" value must be array"#, key))
            .iter()
            .map(|v: &Value| {
                v.as_str()
                    .unwrap_or_else(|| panic!(r#"items of "{}" value must be string"#, key))
                    .to_string()
            })
            .collect(),
    )
}

fn get_commands(setting_toml: &Map<String, Value>) -> Vec<String> {
    get_string_list("run", setting_toml).unwrap_or_else(|| {
        panic!(
            "{}",
            Marker::minus(r#""attest.toml" must have "run" value"#)
        )
    })
}

fn get_test_command(setting_toml: &Map<String, Value>) -> Option<Vec<String>> {
    get_string_list("test", setting_toml)
}

#[fastout]
async fn tester(
    examples: &[IO],
    setting_toml: &Map<String, Value>,
    time_limit: u128,
    example_num: Vec<usize>,
    p_build: bool,
) -> Option<Vec<Option<Res>>> {
    let dir: PathBuf = current_dir().unwrap();

    let mut results: Vec<Option<Res>> = Vec::new();

    if let Err(e) = build_wrap(setting_toml, &dir, &mut results, p_build) {
        println!("{}", e);
        return None;
    }

    let commands: Vec<String> = get_commands(setting_toml);

    let execute_command: String = commands
        .first()
        .unwrap_or_else(|| panic!("{}", Marker::minus(r#""command" value is not satisfied"#)))
        .to_owned();

    let args: Vec<String> = if commands.len() > 1 {
        commands[1..].to_vec()
    } else {
        Vec::new()
    };

    let test_commands: Option<Vec<String>> = get_test_command(setting_toml);

    let mut handles = Vec::new();

    for (index, io) in examples.iter().enumerate() {
        if !example_num.is_empty() && !example_num.contains(&(index + 1)) {
            handles.push(None);
            continue;
        }

        let io: IO = io.clone();
        let args: Vec<String> = args.clone();

        let test_commands: Option<Vec<String>> = test_commands.clone();
        let dir: PathBuf = dir.clone();
        let execute_command: String = execute_command.clone();

        let f = async move {
            let mut buf: String = String::new();

            writeln!(buf, "{} \x1b[35mexample{}\x1b[m", Marker::X, index + 1)?;

            let output = spawn_command(&io.input, &dir, &execute_command, &args[..]).await?;

            let start: Instant = Instant::now();

            let output: Output =
                match time::timeout(Duration::from_millis(time_limit as u64), output).await {
                    Ok(v) => v?,
                    Err(_) => {
                        let time: u128 = start.elapsed().as_millis();

                        writeln!(buf, "{} \x1b[33mTLE\x1b[m\n", Marker::Minus)?;

                        writeln!(buf, "{} input:\n{}", Marker::X, io.input)?;
                        writeln!(buf, "{} expect output:\n{}", Marker::X, io.output)?;

                        writeln!(buf, "{} time: {}", Marker::X, time)?;

                        return Result::<(Res, String)>::Ok((Res::TLE, buf));
                    }
                };

            let time: u128 = start.elapsed().as_millis();

            Ok((
                check(output, time, &io, &test_commands, &dir, &mut buf).await?,
                buf,
            ))
        };

        handles.push(Some(tokio::spawn(f)))
    }

    for op_handle in handles {
        if let Some(handle) = op_handle {
            match handle.await.unwrap() {
                Ok((res, test_result)) => {
                    results.push(Some(res));
                    println!("{}", test_result);
                }
                Err(err) => {
                    results.push(None);
                    eprintln!("{} \x1b[32mError\x1b[m", Marker::Minus);
                    eprintln!("{} Error message or detail", Marker::X);
                    eprintln!("{}", err)
                }
            }
        } else {
            results.push(None);
        }
    }

    for (i, r) in results.iter().enumerate() {
        if let Some(r) = r {
            println!(
                "{} example{}: {}",
                Marker::from(r),
                i + 1,
                match r {
                    Res::AC => "\x1b[32mAC\x1b[m",
                    Res::WA => "\x1b[33mWA\x1b[m",
                    Res::RE => "\x1b[33mRE\x1b[m",
                    Res::TLE => "\x1b[33mTLE\x1b[m",
                    _ => "",
                }
            );
        }
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

async fn spawn_command<T: AsRef<Path>>(
    input: &str,
    dir: T,
    execute_command: &str,
    args: &[String],
) -> Result<impl Future<Output = Result<Output, std::io::Error>>> {
    let mut child: Child = Command::new(execute_command)
        .kill_on_drop(true)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(dir)
        .spawn()?;

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .await?;

    Ok(child.wait_with_output())
}

async fn custom_judge<T: AsRef<Path>>(
    test_command: &Option<Vec<String>>,
    result: &str,
    io: &IO,
    dir: T,
) -> Result<(bool, Option<String>)> {
    let Some(command) = test_command.as_ref() else {
        return Ok((false, None));
    };

    let mut args = command[1..].to_vec();
    args.push(result.to_string());
    args.push(io.output.clone());

    let test_output: Output = spawn_command(
        &format!("{}\n{}", result, &io.output),
        dir,
        command
            .first()
            .with_context(|| "The running command is not set")?,
        &args[..],
    )
    .await?
    .await?;

    if test_output.status.code() != Some(0) {
        let mut error_message = String::new();
        writeln!(
            &mut error_message,
            "{} The test command failed",
            Marker::Minus
        )?;
        writeln!(&mut error_message, "{} Error message", Marker::X)?;
        writeln!(
            &mut error_message,
            "{}",
            String::from_utf8(test_output.stderr)?
        )?;
        bail!(error_message)
    }

    let mut judge = std::str::from_utf8(&test_output.stdout)
        .unwrap_or("")
        .split('\n');

    let judge_res = judge.next().unwrap_or("");

    Ok((
        match judge_res {
            "true" => true,
            "false" => false,
            _ => bail!(
                "{} Output format is wrong. The first line of output must be \"true\" or \"false\"",
                Marker::Minus
            ),
        },
        judge.next().map(|x| x.to_string()),
    ))
}

async fn check<T: AsRef<Path>>(
    output: Output,
    time: u128,
    io: &IO,
    test_command: &Option<Vec<String>>,
    dir: T,
    buf: &mut String,
) -> Result<Res> {
    let result: &str = std::str::from_utf8(&output.stdout).unwrap_or("");

    let return_value: Res = if output.status.code() == Some(0) {
        let (condition, discription): (bool, Option<String>) =
            if test_command.is_some() && !test_command.as_ref().unwrap().is_empty() {
                custom_judge(test_command, result, io, dir).await?
            } else {
                (result == io.output, None)
            };

        let print_discription = |buf: &mut String| -> Result<()> {
            if let Some(d) = discription {
                writeln!(buf, "{} discription:\n{}", Marker::X, d)?;
            }
            Ok(())
        };

        if condition {
            writeln!(buf, "{} \x1b[32mAC\x1b[m", Marker::Plus)?;
            print_discription(buf)?;
            writeln!(buf)?;
            writeln!(buf, "{} input:\n{}", Marker::X, io.input)?;
            Res::AC
        } else {
            writeln!(buf, "{} \x1b[33mWA\x1b[m", Marker::Minus)?;
            print_discription(buf)?;

            writeln!(buf)?;

            writeln!(buf, "{} input:\n{}", Marker::X, io.input)?;
            writeln!(buf, "{} excepted output:\n{}", Marker::X, io.output)?;
            Res::WA
        }
    } else {
        writeln!(buf, "{} \x1b[33mRE\x1b[m", Marker::Minus)?;
        writeln!(buf, "{} input:\n{}", Marker::X, io.input)?;
        Res::RE
    };

    writeln!(buf, "{} output:\n{}\n", Marker::X, result)?;

    writeln!(
        buf,
        "{} stderr:\n{}",
        Marker::X,
        std::str::from_utf8(&output.stderr)?
    )?;

    writeln!(buf, "{} time: {}", Marker::X, time)?;
    writeln!(buf)?;

    Ok(return_value)
}
