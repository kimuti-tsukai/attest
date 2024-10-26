use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::io::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use dirs::home_dir;
use reqwest::cookie::{Cookie, Jar};
use reqwest::{Client, Response};
use scraper::{ElementRef, Html, Selector};
use toml::Value;

use crate::utils::{
    create_err, file_read_to_string, get_item_toml, lang_select, make_client, request,
    set_item_toml, to_html, write_err, Marker,
};

// Initialize
pub fn init() {
    File::create("./attest.toml").unwrap_or_else(|_| panic!("{}", create_err("./attest.toml")));

    create_dir_all("./.attest").unwrap_or_else(|_| panic!("{}", create_err("./.attest")));

    File::create("./.attest/before.txt")
        .unwrap_or_else(|_| panic!("{}", create_err("./.attest/before.txt")));

    File::create("./.attest/before_setting.toml")
        .unwrap_or_else(|_| panic!("{}", create_err("./.attest/before_setting.toml")));

    File::create("./.attest/url.txt")
        .unwrap_or_else(|_| panic!("{}", create_err("./.attest/url.txt")));

    File::create("./.attest/examples.json")
        .unwrap_or_else(|_| panic!("{}", create_err("./.attest/examples.json")));

    File::create("./.attest/deps_caches.json")
        .unwrap_or_else(|_| panic!("{}", create_err("./.attest/deps_caches.json")));

    File::create("./.attest/time_limit.txt")
        .unwrap_or_else(|_| panic!("{}", create_err("./.attest/time_limit.txt")));

    println!("{}", Marker::plus("\x1b[32mFinished successfully\x1b[m"));
}

// Show settings
pub fn show_set() {
    let r = file_read_to_string("./attest.toml");

    println!("{}", &r.trim_end());
}

// Set the build command
pub fn set_build(commands: Vec<String>) {
    let values: Vec<Value> = commands
        .into_iter()
        .map(|v: String| Value::String(v))
        .collect();

    set_item_toml("./attest.toml", "build", Value::Array(values));

    println!("{}", Marker::plus("\x1b[32mFinished successfully\x1b[m"));
}

// Set the run command
pub fn set_run(commands: Vec<String>) {
    let values: Vec<Value> = commands
        .into_iter()
        .map(|v: String| Value::String(v))
        .collect();

    set_item_toml("./attest.toml", "run", Value::Array(values));

    println!("{}", Marker::plus("\x1b[32mFinished successfully\x1b[m"));
}

// Set the test command
pub fn set_test(commands: Vec<String>) {
    let values: Vec<Value> = commands
        .into_iter()
        .map(|v: String| Value::String(v))
        .collect();

    set_item_toml("./attest.toml", "test", Value::Array(values));

    println!("{}", Marker::plus("\x1b[32mFinished successfully\x1b[m"));
}

// Set the program file
pub fn set_file(name: String) {
    set_item_toml("./attest.toml", "file_path", Value::String(name));

    println!("{}", Marker::plus("\x1b[32mFinished successfully\x1b[m"));
}

// Set the other depends file
pub fn set_deps_file(paths: Vec<String>, add: bool) {
    let mut now = if add {
        get_item_toml("./attest.toml", "deps_files")
            .unwrap_or(Value::Array(Vec::new()))
            .as_array()
            .unwrap_or(&Vec::new())
            .to_owned()
    } else {
        Vec::new()
    };

    let mut list: Vec<Value> = paths.iter().cloned().map(Value::String).collect();

    now.append(&mut list);

    set_item_toml("./attest.toml", "deps_files", Value::Array(now));

    println!("{}", Marker::plus("\x1b[32mFinished successfully\x1b[m"));
}

pub async fn login(user_name: String, password: String) {
    let url: &str = "https://atcoder.jp/login?continue=https://atcoder.jp/";

    let cookies: Arc<Jar> = Arc::new(Jar::default());

    let client: Client = Client::builder()
        .cookie_store(true)
        .cookie_provider(cookies)
        .build()
        .unwrap();

    let page: Response = client.get(url).send().await.unwrap();

    let text: String = page.text().await.unwrap();

    let html: Html = Html::parse_document(&text);

    let selector: Selector = Selector::parse(r#"input[name="csrf_token"]"#).unwrap();

    let csrf_token: &str = html
        .select(&selector)
        .next()
        .unwrap()
        .attr("value")
        .unwrap();

    let mut form: HashMap<&str, &str> = HashMap::new();

    form.insert("username", &user_name);
    form.insert("password", &password);
    form.insert("csrf_token", csrf_token);

    let response: Response = client.post(url).form(&form).send().await.unwrap();

    if response.url().path() == "/" {
        let cookies: Vec<Cookie> = response.cookies().collect();

        let login_cookie: &Cookie = cookies
            .iter()
            .find(|&v: &&Cookie| v.name() == "REVEL_SESSION")
            .unwrap();

        let cookie_value: &str = login_cookie.value();

        let mut dir: PathBuf = dirs::home_dir().unwrap();

        dir.push(".attest_global");

        if !dir.is_dir() {
            create_dir_all(&dir).unwrap_or_else(|_| panic!("{}", create_err(&dir)));
        }

        dir.push("cookies.txt");

        let mut file: File = File::create(&dir).unwrap_or_else(|_| panic!("{}", create_err(&dir)));

        writeln!(&mut file, "REVEL_SESSION = {}", cookie_value)
            .unwrap_or_else(|_| panic!("{}", write_err(dir)));

        println!("{}", Marker::plus("\x1b[32mFinished successfully\x1b[m"));
    } else {
        println!("{}", Marker::minus("\x1b[31mFailed to login\x1b[m"));
    }
}

pub fn logout() {
    let mut dir: PathBuf = home_dir().unwrap();

    dir.push(".attest_global/cookies.txt");

    File::create(&dir).unwrap_or_else(|_| panic!("{}", create_err(dir)));
}

pub async fn lang(
    lang: Option<String>,
    list: bool,
    url: Option<String>,
    search: Option<String>,
) -> Result<()> {
    if lang.is_none() && !list && search.is_none() {
        panic!("{} The lang command must have arguments", Marker::Minus);
    }

    let client: Client = make_client();

    let langs: Vec<(String, String)> = match url {
        Some(u) => {
            let url: String = u;

            let text: String = request(&client, &url).await.unwrap();

            let html: Html = to_html(text);

            lang_select(&html)
        }
        None => {
            let past_contests: String = request(
                &client,
                "https://atcoder.jp/contests/archive?ratedType=1&category=0&keyword=",
            )
            .await?;

            let html: Html = to_html(past_contests);

            let selector: Selector =
                Selector::parse(r#"div[class="table-responsive"] tbody td span + a"#).unwrap();

            let url_elem: ElementRef = html.select(&selector).next().unwrap();

            let url: String =
                String::from("https://atcoder.jp") + url_elem.attr("href").unwrap() + "/submit";

            let text: String = request(&client, &url).await.unwrap();

            let html: Html = to_html(text);

            let selector: Selector =
                Selector::parse(r#"label[for="select-lang"] + div select"#).unwrap();

            let selected: Html = to_html(html.select(&selector).next().unwrap().html());

            lang_select(&selected)
        }
    };

    if let Some(lang_name) = search {
        for (lang, _) in &langs {
            if lang
                .to_ascii_lowercase()
                .contains(&lang_name.to_ascii_lowercase())
            {
                println!("{}", lang);
            }
        }
    } else if list {
        for (lang, _) in &langs {
            println!("{}", lang);
        }
    } else if let Some(lang_name) = lang {
        let lang_code: &str = &langs
            .iter()
            .find(|&v: &&(String, String)| v.0 == lang_name)
            .unwrap_or_else(|| panic!("{}", Marker::minus("The lang cannot be used")))
            .1;
        set_item_toml("./attest.toml", "lang", Value::String(lang_code.to_owned()));
        println!("{}", Marker::plus("\x1b[32mFinished successfully\x1b[m"));
    } else {
        panic!("{}", Marker::minus("Arguments may be wrong format"));
    }

    Ok(())
}
