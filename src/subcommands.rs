use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::io::prelude::*;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use dirs::home_dir;
use reqwest::cookie::{Cookie, Jar};
use reqwest::Client;
use scraper::{Html, Selector};
use toml::Value;

use crate::utils::{
    lang_select, make_client, request, set_item_toml, to_html, CREATE_ERR, OPEN_ERR, READ_ERR,
    WRITE_ERR,
};

// Initialize
pub fn init() {
    File::create("./test.toml").expect("something went wrong creating a file");

    create_dir_all("./.attest").expect(CREATE_ERR);

    File::create("./.attest/before.txt").expect(CREATE_ERR);

    File::create("./.attest/cache.toml").expect(CREATE_ERR);

    File::create("./.attest/url.txt").expect(CREATE_ERR);

    File::create("./.attest/examples.json").expect(CREATE_ERR);

    File::create("./.attest/time_limit.txt").expect(CREATE_ERR);

    println!("\x1b[32mFinished successfully\x1b[m");
}

// Show settings
pub fn show_set() {
    let mut r = String::new();
    File::open("./test.toml")
        .expect(OPEN_ERR)
        .read_to_string(&mut r)
        .expect(READ_ERR);

    println!("{}", &r.trim_end());
}

// Set the build command
pub fn set_build(commands: Vec<String>) {
    let values: Vec<Value> = commands.iter().map(|v| Value::String(v.clone())).collect();

    set_item_toml("./test.toml", "build", Value::Array(values));

    println!("\x1b[32mFinished successfully\x1b[m");
}

// Set the run command
pub fn set_run(commands: Vec<String>) {
    let values: Vec<Value> = commands.iter().map(|v| Value::String(v.clone())).collect();

    set_item_toml("./test.toml", "run", Value::Array(values));

    println!("\x1b[32mFinished successfully\x1b[m");
}

// Set the test command
pub fn set_test(commands: Vec<String>) {
    let values: Vec<Value> = commands.iter().map(|v| Value::String(v.clone())).collect();

    set_item_toml("./test.toml", "test", Value::Array(values));

    println!("\x1b[32mFinished successfully\x1b[m");
}

// Set the program file
pub fn set_file(name: String) {
    set_item_toml("./test.toml", "file_path", Value::String(name));

    println!("\x1b[32mFinished successfully\x1b[m");
}

pub async fn login(user_name: String, password: String) {
    let url = "https://atcoder.jp/login?continue=https://atcoder.jp/";

    let cookies = Arc::new(Jar::default());

    let client = Client::builder()
        .cookie_store(true)
        .cookie_provider(cookies)
        .build()
        .unwrap();

    let page = client.get(url).send().await.unwrap();

    let text = page.text().await.unwrap();

    let html = Html::parse_document(&text);

    let selector = Selector::parse(r#"input[name="csrf_token"]"#).unwrap();

    let csrf_token = html
        .select(&selector)
        .next()
        .unwrap()
        .attr("value")
        .unwrap();

    let mut form: HashMap<&str, &str> = HashMap::new();

    form.insert("username", &user_name);
    form.insert("password", &password);
    form.insert("csrf_token", csrf_token);

    let response = client.post(url).form(&form).send().await.unwrap();

    if response.url().path() == "/" {
        let cookies: Vec<Cookie> = response.cookies().collect();

        let login_cookie = cookies
            .iter()
            .find(|&v| v.name() == "REVEL_SESSION")
            .unwrap();

        let cookie_value = login_cookie.value();

        let home_dir = dirs::home_dir().unwrap();
        let home_dir_txt = home_dir.to_str().unwrap();

        if !Path::new(&format!("{}/.attest_global", home_dir_txt)).is_dir() {
            create_dir_all(format!("{}/.attest_global", home_dir_txt)).expect(CREATE_ERR);
        }

        let mut file =
            File::create(format!("{}/.attest_global/cookies.txt", home_dir_txt)).expect(CREATE_ERR);

        writeln!(&mut file, "REVEL_SESSION = {}", cookie_value).expect(WRITE_ERR);

        println!("\x1b[32mFinished successfully\x1b[m");
    } else {
        panic!("\x1b[31mFailed to login\x1b[m");
    }
}

pub fn logout() {
    let home_dir_path = home_dir().unwrap();
    let home_dir = home_dir_path.to_str().unwrap();

    File::create(format!("{}/.attest_global/cookies.txt", home_dir)).expect(CREATE_ERR);
}

pub async fn lang(
    lang: Option<String>,
    list: bool,
    url: Option<String>,
    search: Option<String>,
) -> Result<()> {
    if lang.is_none() && !list && search.is_none() {
        panic!("The lang command must have arguments");
    }

    let client = make_client();

    let langs = match url {
        Some(u) => {
            let url = u;

            let text = request(&client, &url).await.unwrap();

            let html = to_html(text);

            lang_select(&html)
        }
        None => {
            let past_contests = request(
                &client,
                "https://atcoder.jp/contests/archive?ratedType=1&category=0&keyword=",
            )
            .await?;

            let html = to_html(past_contests);

            let selector =
                Selector::parse(r#"div[class="table-responsive"] tbody td span + a"#).unwrap();

            let url_elem = html.select(&selector).next().unwrap();

            let url =
                String::from("https://atcoder.jp") + url_elem.attr("href").unwrap() + "/submit";

            let text = request(&client, &url).await.unwrap();

            let html = to_html(text);

            let selector = Selector::parse(r#"label[for="select-lang"] + div select"#).unwrap();

            let selected = to_html(html.select(&selector).next().unwrap().html());

            lang_select(&selected)
        }
    };

    if list {
        for (lang, _) in &langs {
            println!("{}", lang);
        }
    } else if let Some(lang_name) = search {
        for (lang, _) in &langs {
            if lang
                .to_ascii_lowercase()
                .contains(&lang_name.to_ascii_lowercase())
            {
                println!("{}", lang);
            }
        }
    } else if let Some(lang_name) = lang {
        let lang_code = &langs
            .iter()
            .find(|&v| v.0 == lang_name)
            .expect("The lang cannot be used")
            .1;
        set_item_toml("./test.toml", "lang", Value::String(lang_code.to_owned()));
        println!("\x1b[32mFinished successfully\x1b[m");
    } else {
        panic!("Arguments may be wrong format");
    }

    Ok(())
}

/*
// Show the help
pub fn help() {
    let text = r#"
    This is the tool for atcoder

    This tests your code with examples which are on atcoder page

    You can run with "attest url" command

    "attest url example_number" command can test the example number

    Commands:
        set build commands          setting building command
        set run commands            setting running command
        set file file_path          setting program file path
        help                        show help

    Test command must be satisfied with below:
        Input:
            ```
            R C
            ```
            R: Result the executing the run command
            C: Correct answer
            You can receive input as either stdin or command line arguments

        Output:
            If judge is correct answer return `true` else `false`
"#;
    println!("{}", text);
}
*/
