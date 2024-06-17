use std::io::prelude::*;
use std::{fs::File, sync::Arc};

use anyhow::Result;
use reqwest::cookie::Jar;
use reqwest::{Client, Url};
use scraper::{Html, Selector};
use toml::{map::Map, Table, Value};

pub const OPEN_ERR: &str = "something went wrong opening a file";
pub const READ_ERR: &str = "something went wrong reading a file";
pub const CREATE_ERR: &str = "something went wrong creating a file or directory";
pub const WRITE_ERR: &str = "something went wrong writing to a file";

// Request the link
pub async fn request(client: &Client, url: &str) -> Result<String> {
    let response = client.get(url).send().await?;

    let body = response.text().await?;

    Ok(body)
}

// Convert `String` to `Html`
pub fn to_html(text: String) -> Html {
    Html::parse_document(&text)
}

pub fn make_client() -> Client {
    let cookies = Arc::new(Jar::default());

    let cookies_string = get_cookies();

    cookies.add_cookie_str(&cookies_string, &Url::parse("https://atcoder.jp").unwrap());

    Client::builder()
        .cookie_store(true)
        .cookie_provider(cookies)
        .build()
        .unwrap()
}

pub fn get_cookies() -> String {
    let home_dir = dirs::home_dir().unwrap();
    let home_dir_txt = home_dir.to_str().unwrap();

    file_read_to_string(&format!("{}/.attest_global/cookies.txt", home_dir_txt))
}

pub fn set_item_toml(path: &str, key: &str, value: Value) {
    let setting = file_read_to_string(path);

    let mut f = File::create(path).expect(CREATE_ERR);

    let mut setting_toml = setting
        .parse::<Table>()
        .expect(r#""test.toml" has wrong format"#);

    setting_toml.insert(String::from(key), value);

    write!(&mut f, "{}", setting_toml).expect(WRITE_ERR);
}

pub fn lang_select(html: &Html) -> Vec<(String, String)> {
    let selector = Selector::parse(r#"option"#).unwrap();

    let selected = html.select(&selector);

    selected
        .skip(1)
        .map(|i| {
            (
                i.text().next().unwrap().to_string(),
                i.attr("value").unwrap().to_string(),
            )
        })
        .collect()
}

pub fn items_toml(path: &str) -> Map<String, Value> {
    let text = file_read_to_string(path);

    text.parse::<Table>()
        .unwrap_or_else(|_| panic!(r#""{}" has wrong format"#, path))
}

pub fn get_item_toml(path: &str, key: &str) -> Option<Value> {
    let items = items_toml(path);

    Some(items.get(key)?.to_owned())
}

pub fn file_read_to_string(path: &str) -> String {
    let mut f = String::new();
    File::open(path)
        .expect(OPEN_ERR)
        .read_to_string(&mut f)
        .expect(READ_ERR);

    f
}
