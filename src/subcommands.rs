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
    lang_select, make_client, request, set_item_toml, to_html, Marker, CREATE_ERR, OPEN_ERR,
    READ_ERR, WRITE_ERR,
};

// Initialize
pub fn init() {
    File::create("./attest.toml").expect(CREATE_ERR);

    create_dir_all("./.attest").expect(CREATE_ERR);

    File::create("./.attest/before.txt").expect(CREATE_ERR);

    File::create("./.attest/cache.toml").expect(CREATE_ERR);

    File::create("./.attest/url.txt").expect(CREATE_ERR);

    File::create("./.attest/examples.json").expect(CREATE_ERR);

    File::create("./.attest/time_limit.txt").expect(CREATE_ERR);

    println!("{}", Marker::plus("\x1b[32mFinished successfully\x1b[m"));
}

// Show settings
pub fn show_set() {
    let mut r: String = String::new();
    File::open("./attest.toml")
        .expect(OPEN_ERR)
        .read_to_string(&mut r)
        .expect(READ_ERR);

    println!("{}", &r.trim_end());
}

// Set the build command
pub fn set_build(commands: Vec<String>) {
    let values: Vec<Value> = commands
        .iter()
        .map(|v: &String| Value::String(v.clone()))
        .collect();

    set_item_toml("./attest.toml", "build", Value::Array(values));

    println!("{}", Marker::plus("\x1b[32mFinished successfully\x1b[m"));
}

// Set the run command
pub fn set_run(commands: Vec<String>) {
    let values: Vec<Value> = commands
        .iter()
        .map(|v: &String| Value::String(v.clone()))
        .collect();

    set_item_toml("./attest.toml", "run", Value::Array(values));

    println!("{}", Marker::plus("\x1b[32mFinished successfully\x1b[m"));
}

// Set the test command
pub fn set_test(commands: Vec<String>) {
    let values: Vec<Value> = commands
        .iter()
        .map(|v: &String| Value::String(v.clone()))
        .collect();

    set_item_toml("./attest.toml", "test", Value::Array(values));

    println!("{}", Marker::plus("\x1b[32mFinished successfully\x1b[m"));
}

// Set the program file
pub fn set_file(name: String) {
    set_item_toml("./attest.toml", "file_path", Value::String(name));

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

        if dir.is_dir() {
            create_dir_all(&dir).expect(CREATE_ERR);
        }

        dir.push("cookies.txt");

        let mut file: File = File::create(&dir).expect(CREATE_ERR);

        writeln!(&mut file, "REVEL_SESSION = {}", cookie_value).expect(WRITE_ERR);

        println!("{}", Marker::plus("\x1b[32mFinished successfully\x1b[m"));
    } else {
        println!("{}", Marker::minus("\x1b[31mFailed to login\x1b[m"));
    }
}

pub fn logout() {
    let mut dir: PathBuf = home_dir().unwrap();

    dir.push(".attest_global/cookies.txt");

    File::create(dir).expect(CREATE_ERR);
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
