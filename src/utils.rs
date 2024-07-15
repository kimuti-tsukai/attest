use std::fmt::Debug;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::{
    fs::{read_to_string, File},
    sync::Arc,
};

use anyhow::Result;
use reqwest::cookie::Jar;
use reqwest::{Client, Response, Url};
use scraper::html::Select;
use scraper::{ElementRef, Html, Selector};
use toml::{map::Map, Table, Value};

pub const OPEN_ERR: &str = "something went wrong opening a file";
pub const READ_ERR: &str = "something went wrong reading a file";
pub const CREATE_ERR: &str = "something went wrong creating a file or directory";
pub const WRITE_ERR: &str = "something went wrong writing to a file";

// Request the link
pub async fn request(client: &Client, url: &str) -> Result<String> {
    let response: Response = client.get(url).send().await?;

    let body: String = response.text().await?;

    Ok(body)
}

// Convert `String` to `Html`
pub fn to_html(text: String) -> Html {
    Html::parse_document(&text)
}

pub fn make_client() -> Client {
    let cookies: Arc<Jar> = Arc::new(Jar::default());

    let cookies_string: String = get_cookies();

    cookies.add_cookie_str(&cookies_string, &Url::parse("https://atcoder.jp").unwrap());

    Client::builder()
        .cookie_store(true)
        .cookie_provider(cookies)
        .build()
        .unwrap()
}

pub fn get_cookies() -> String {
    let mut dir: PathBuf = dirs::home_dir().unwrap();

    dir.push(".attest_global/cookies.txt");

    file_read_to_string(dir)
}

pub fn set_item_toml<T: AsRef<Path>>(path: T, key: &str, value: Value) {
    let setting: String = file_read_to_string(&path);

    let mut f: File = File::create(&path).expect(CREATE_ERR);

    let mut setting_toml: Map<String, Value> = setting
        .parse::<Table>()
        .expect(r#""test.toml" has wrong format"#);

    setting_toml.insert(String::from(key), value);

    write!(&mut f, "{}", setting_toml).expect(WRITE_ERR);
}

pub fn lang_select(html: &Html) -> Vec<(String, String)> {
    let selector: Selector = Selector::parse(r#"option"#).unwrap();

    let selected: Select = html.select(&selector);

    selected
        .skip(1)
        .map(|i: ElementRef| {
            (
                i.text().next().unwrap().to_string(),
                i.attr("value").unwrap().to_string(),
            )
        })
        .collect()
}

pub fn items_toml<T: AsRef<Path> + Debug>(path: T) -> Map<String, Value> {
    let text: String = file_read_to_string(&path);

    text.parse::<Table>()
        .unwrap_or_else(|_| panic!("{:?} has wrong format", path))
}

pub fn get_item_toml<T: AsRef<Path> + Debug>(path: T, key: &str) -> Option<Value> {
    let items: Map<String, Value> = items_toml(path);

    Some(items.get(key)?.to_owned())
}

pub fn file_read_to_string<T: AsRef<Path> + Clone>(path: T) -> String {
    read_to_string(path.clone()).unwrap_or_else(|_| {
        panic!(
            "{} file does not exist",
            path.as_ref().as_os_str().to_str().unwrap_or("")
        )
    })
}

pub fn link_from_copy() -> String {
    file_read_to_string("./.attest/url.txt")
}
