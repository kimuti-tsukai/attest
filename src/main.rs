use anyhow::Result;
use std::collections::HashMap;
use std::env::current_dir;
use std::fs::File;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::num::IntErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Output, Stdio};
use std::time::{Duration, Instant};

mod subcommands;
mod utils;

use utils::{
    file_read_to_string, get_item_toml, items_toml, lang_select, make_client, request, to_html,
    CREATE_ERR, WRITE_ERR,
};

use rustc_hash::FxHasher;
use tokio::process::Command;
use tokio::time;
use toml::{map::Map, Value};

use serde::{Deserialize, Serialize};

use scraper::{Html, Selector};

use regex::Regex;

use clap::{Parser, Subcommand};

/// Tester for AtCoder examples.
/// This tests your program in the example cases
#[derive(Parser)]
enum Arg {
    /// Test the examples
    #[clap(visible_alias("t"))]
    Test {
        #[arg(help = "URL of AtCoder")]
        url: Option<String>,

        #[clap(short = 'n',long = "num",num_args = 0..,value_delimiter = ' ')]
        example_num: Vec<usize>,
    },
    /// Submit your code
    #[clap(visible_alias("s"))]
    Submit {
        url: Option<String>,

        #[clap(short = 'l', long = "lang")]
        lang: Option<String>,
    },
    /// Test and Submit if all tests get AC
    #[clap(visible_alias("ts"))]
    Tebmit {
        url: Option<String>,

        #[clap(short = 'l', long = "lang")]
        lang: Option<String>,
    },
    /// Show or Select langs
    #[clap(visible_alias("l"))]
    Lang {
        lang: Option<String>,

        #[clap(short = 'l', long = "list")]
        list: bool,

        #[clap(short = 'u', long = "url")]
        url: Option<String>,

        #[clap(short = 's', long = "search")]
        search: Option<String>,
    },
    /// Init the environment to test
    Init,
    /// Set the environment to test
    Set {
        #[command(subcommand)]
        command: Option<Sets>,
    },
    Login {
        #[arg()]
        user_name: String,

        #[arg()]
        password: String,
    },
    Logout,
}

#[derive(Subcommand)]
enum Sets {
    /// Set the build command
    Build {
        #[clap(value_delimiter = ' ')]
        command: Vec<String>,
    },
    /// Set the run command
    Run {
        #[clap(value_delimiter = ' ')]
        command: Vec<String>,
    },
    #[command(about = r#"Set the test command
Test command must be satisfied with below:
Input:
    ```
    R C
    ```
    R: Result the executing the run command
    C: Correct answer
    You can receive input as either stdin or command line arguments

Output:
    If judge is correct answer return `true` else `false`"#)]
    Test {
        #[clap(value_delimiter = ' ')]
        command: Vec<String>,
    },
    /// Set the program file
    File { file_path: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Arg::parse();

    match args {
        Arg::Test { url, example_num } => { run(url, example_num).await?; },
        Arg::Submit { url, lang } => { submit(url, lang).await; },
        Arg::Tebmit { url, lang } => {
            let results = run(url.clone(), Vec::new()).await?;

            if let Some(v) = results {
                if v.iter().all(|&a| a == Res::AC) {
                    submit(url, lang).await;
                }
            }
        }
        Arg::Lang {
            lang,
            list,
            url,
            search,
        } => subcommands::lang(lang, list, url, search).await?,
        Arg::Init => subcommands::init(),
        Arg::Set { command } => {
            if let Some(command) = command {
                match command {
                    Sets::Build { command } => subcommands::set_build(command),
                    Sets::Run { command } => subcommands::set_run(command),
                    Sets::Test { command } => subcommands::set_test(command),
                    Sets::File { file_path } => subcommands::set_file(file_path),
                }
            } else {
                subcommands::show_set();
            }
        }
        Arg::Login {
            user_name,
            password,
        } => subcommands::login(user_name, password).await,
        Arg::Logout => subcommands::logout(),
    }

    Ok(())
}

// Function to run
async fn run(url: Option<String>, example_num: Vec<usize>) -> Result<Option<Vec<Res>>> {
    let (examples, time_limit);

    if url.is_none() || is_same_link(&url.clone().unwrap()) {
        examples = examples_from_cache();

        time_limit = time_limit_from_cache();
    } else {
        let url = url.unwrap();

        let c = make_client();

        let text = request(&c, &url).await?;

        let html = to_html(text);

        examples = assort(&select_samples(&html));

        time_limit = get_time_limit(&html);

        save_cache(&url, time_limit, &examples);
    }
    let setting_toml = items_toml("./test.toml");

    let results = test(&examples, &setting_toml, time_limit, example_num).await;

    Ok(results)
}

// Check if the link is same
fn is_same_link(url: &str) -> bool {
    url == link_from_copy()
}

fn link_from_copy() -> String {
    file_read_to_string("./.attest/url.txt")
}

// Get examples from cache if the link is same
fn examples_from_cache() -> Vec<IO> {
    let text = file_read_to_string("./.attest/examples.json");

    serde_json::from_str(&text).unwrap()
}

// Get time limit from cache if the link is same
fn time_limit_from_cache() -> u128 {
    let time = file_read_to_string("./.attest/time_limit.txt");

    time.parse().unwrap()
}

// Save cache if the link is different
fn save_cache(url: &str, time_limit: u128, examples: &Vec<IO>) {
    let mut l = File::create("./.attest/url.txt").expect(CREATE_ERR);
    writeln!(&mut l, "{}", url).expect(WRITE_ERR);

    let mut t = File::create("./.attest/time_limit.txt").expect(CREATE_ERR);
    writeln!(&mut t, "{}", time_limit).expect(WRITE_ERR);

    let mut e = File::create("./.attest/examples.json").expect(CREATE_ERR);
    writeln!(&mut e, "{}", serde_json::to_string(examples).unwrap()).expect(WRITE_ERR);
}

// Select examples from Html
fn select_samples(html: &Html) -> Vec<String> {
    let selector = Selector::parse(r#"span[class="lang-ja"] h3 + pre"#).unwrap();

    let mut samples = Vec::new();

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
        .map(|l| IO::new(l[0].clone(), l[1].clone()))
        .collect()
}

// Get time limit
fn get_time_limit(html: &Html) -> u128 {
    let selector = Selector::parse(r#"div[class="col-sm-12"] > p"#).unwrap();

    let t = html
        .select(&selector)
        .next()
        .unwrap()
        .text()
        .next()
        .unwrap();

    let re1 = Regex::new("Time Limit: (.+) sec").unwrap();
    let re2 = Regex::new("Time Limit: (.+) msec").unwrap();

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
    let file_path = setting_toml
        .get("file_path")?
        .as_str()
        .expect(r#"the "file_path" value must be string"#);

    if !Path::new(file_path).is_file() {
        return None;
    }

    let now = file_read_to_string(file_path);

    let mut now_hasher = FxHasher::default();

    now.hash(&mut now_hasher);

    let now_hash = now_hasher.finish();

    let before = file_read_to_string("./.attest/before.txt");

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
        let mut f = File::create("./.attest/before.txt").expect(CREATE_ERR);
        write!(&mut f, "{}", now_hash).expect(WRITE_ERR);
        Some(false)
    }
}

fn is_same_setting(setting_toml: &Map<String, Value>) -> Result<bool> {
    let before_settiing = items_toml("./.attest/cache.toml");

    if setting_toml == &before_settiing {
        Ok(true)
    } else {
        let mut f = File::create("./.attest/cache.toml").expect(CREATE_ERR);
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
            .map(|v| {
                v.as_str()
                    .expect(r#"items of "build" value must be string"#)
            })
            .collect();

        let &command = build_commands.first()?;

        let args = if build_commands.len() > 1 {
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
) -> Result<(),()> {
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
        .map(|v| {
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
            .map(|v| {
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
    let pipe = StdCommand::new("echo")
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

async fn test(
    examples: &[IO],
    setting_toml: &Map<String, Value>,
    time_limit: u128,
    example_num: Vec<usize>,
) -> Option<Vec<Res>> {
    let dir = current_dir().unwrap();

    let mut results: Vec<Res> = Vec::new();

    if build_wrap(setting_toml, &dir, &mut results).is_err() {
        return None;
    }

    let commands: Vec<String> = get_commands(setting_toml);

    let execute_command = commands
        .first()
        .expect(r#""command" value is not satisfied"#);

    let args = if commands.len() > 1 {
        &commands[1..]
    } else {
        &[]
    };

    let test_commands = get_test_command(setting_toml);

    for (index, io) in examples.iter().enumerate() {
        if !example_num.is_empty() && !example_num.contains(&(index + 1)) {
            continue;
        }

        println!("example{}", index + 1);

        let output = spawn_command(io, &dir, execute_command, args);

        let start = Instant::now();

        let output = match time::timeout(Duration::from_millis(time_limit as u64), output).await {
            Ok(v) => {
                if let Ok(v) = v {
                    v
                } else {
                    continue;
                }
            }
            Err(_) => {
                let time = start.elapsed().as_millis();

                println!("\x1b[33mTLE\x1b[m");

                println!();

                println!("input:\n{}", io.input);
                println!("expect output:\n{}", io.output);

                println!("time: {}", time);

                results.push(Res::TLE);
                continue;
            }
        };

        let time = start.elapsed().as_millis();

        check(output, time, io, &test_commands, &dir, &mut results);
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
enum Res {
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
    let command = test_command.as_ref().unwrap();

    let pipe = StdCommand::new("echo")
        .args([result, &io.output])
        .current_dir(dir)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let test_output = StdCommand::new(command.first().unwrap())
        .args([result, &io.output])
        .stdin(Stdio::from(pipe.stdout.unwrap()))
        .current_dir(dir)
        .output()
        .unwrap();

    let judge = std::str::from_utf8(&test_output.stdout).unwrap_or("");

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
    results: &mut Vec<Res>,
) {
    let result: &str = std::str::from_utf8(&output.stdout).unwrap_or("");

    if output.status.code() == Some(0) {
        let condition = if test_command.is_some() && !test_command.as_ref().unwrap().is_empty() {
            spawn_test_command(test_command, result, io, dir)
        } else {
            result == io.output
        };

        if condition {
            println!("\x1b[32mAC\x1b[m");
            println!("input:\n{}", io.input);
            results.push(Res::AC);
        } else {
            println!("\x1b[33mWA\x1b[m");

            println!();

            println!("input:\n{}", io.input);
            println!("excepted output:\n{}", io.output);
            results.push(Res::WA);
        }
    } else {
        println!("\x1b[33mRE\x1b[m\n");
        println!("input:\n{}", io.input);
        results.push(Res::RE);
    }
    println!("output:\n{}", result);

    println!("stderr:\n{}", std::str::from_utf8(&output.stderr).unwrap());

    println!("time: {}", time);
}

// Submit Code
async fn submit(url: Option<String>, lang: Option<String>) {
    let url = match url {
        Some(s) => s,
        None => link_from_copy(),
    };

    let client = make_client();

    let s = request(&client, &url).await.unwrap();

    let html = to_html(s);

    let mut form: HashMap<&str, &str> = HashMap::new(); // Submit form

    let task_screen_name_selrctor =
        Selector::parse(r#"input[name="data.TaskScreenName"]"#).unwrap();

    let task_screen_name = html
        .select(&task_screen_name_selrctor)
        .next()
        .expect("You may not login")
        .attr("value")
        .unwrap();

    form.insert("data.TaskScreenName", task_screen_name);

    let csrf_token_selector = Selector::parse(r#"input[name="csrf_token"]"#).unwrap();

    let csrf_token = html
        .select(&csrf_token_selector)
        .next()
        .unwrap()
        .attr("value")
        .unwrap();

    form.insert("csrf_token", csrf_token);

    let lang_code = match lang {
        None => get_item_toml("./test.toml", "lang")
            .expect("You have to set lang")
            .as_str()
            .unwrap()
            .to_owned(),
        Some(lang_name) => {
            let text = request(&client, &url).await.unwrap();

            let html = to_html(text);

            let langs = lang_select(&html);

            let lang_code = &langs
                .iter()
                .find(|&v| v.0 == lang_name)
                .expect("The lang cannot be used")
                .1;

            lang_code.to_owned()
        }
    };

    form.insert("data.LanguageId", &lang_code);

    let file_path_string =
        get_item_toml("./test.toml", "file_path").expect("You have to set file path");

    let file_path = file_path_string.as_str().unwrap();

    let code = file_read_to_string(file_path);

    form.insert("sourceCode", &code);

    let require_addr_selector =
        Selector::parse(r#"form[class="form-horizontal form-code-submit"]"#).unwrap();

    let require_addr = html
        .select(&require_addr_selector)
        .next()
        .unwrap()
        .attr("action")
        .unwrap();

    let addr = String::from("https://atcoder.jp") + require_addr;

    client.post(&addr).form(&form).send().await.unwrap();
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[tokio::test]
    async fn html() {
        let c = make_client();

        let text = request(&c, "https://atcoder.jp/contests/abc356/tasks/abc356_c")
            .await
            .unwrap();

        eprintln!("{}", &text);
    }

    #[test]
    fn home_dir() {
        std::fs::create_dir_all(format!(
            "{}/.attest_global",
            dirs::home_dir().unwrap().to_str().unwrap()
        ))
        .expect(CREATE_ERR);
    }

    #[tokio::test]
    async fn langs() {
        submit(
            Some("https://atcoder.jp/contests/abc356/tasks/abc356_c".to_string()),
            None,
        )
        .await;
    }
}
