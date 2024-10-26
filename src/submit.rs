use std::collections::HashMap;

use crate::utils::{
    file_read_to_string, get_item_toml, lang_select, link_from_copy, make_client, request, to_html,
    Marker,
};

use reqwest::Client;

use toml::Value;

use scraper::{Html, Selector};

// Submit Code
pub async fn submit(url: Option<String>, lang: Option<String>) {
    let url: String = match url {
        Some(s) => s,
        None => link_from_copy(),
    };

    let client: Client = make_client();

    let s: String = request(&client, &url).await.unwrap();

    let html: Html = to_html(s);

    let mut form: HashMap<&str, &str> = HashMap::new(); // Submit form

    let task_screen_name_selrctor: Selector =
        Selector::parse(r#"input[name="data.TaskScreenName"]"#).unwrap();

    let task_screen_name: &str = html
        .select(&task_screen_name_selrctor)
        .next()
        .unwrap_or_else(|| panic!("{}", Marker::minus("You may not login")))
        .attr("value")
        .unwrap();

    form.insert("data.TaskScreenName", task_screen_name);

    let csrf_token_selector: Selector = Selector::parse(r#"input[name="csrf_token"]"#).unwrap();

    let csrf_token: &str = html
        .select(&csrf_token_selector)
        .next()
        .unwrap()
        .attr("value")
        .unwrap();

    form.insert("csrf_token", csrf_token);

    let lang_code: String = match lang {
        None => get_item_toml("./attest.toml", "lang")
            .unwrap_or_else(|| panic!("{}", Marker::minus("You have to set lang")))
            .as_str()
            .unwrap()
            .to_owned(),
        Some(lang_name) => {
            let text: String = request(&client, &url).await.unwrap();

            let html: Html = to_html(text);

            let langs: Vec<(String, String)> = lang_select(&html);

            let lang_code: &str = &langs
                .iter()
                .find(|&v: &&(String, String)| v.0 == lang_name)
                .unwrap_or_else(|| panic!("{}", Marker::minus("The lang cannot be used")))
                .1;

            lang_code.to_string()
        }
    };

    form.insert("data.LanguageId", &lang_code);

    let file_path_string: Value = get_item_toml("./attest.toml", "file_path")
        .unwrap_or_else(|| panic!("{}", Marker::minus("You have to set file path")));

    let file_path: &str = file_path_string.as_str().unwrap();

    let code: String = file_read_to_string(file_path);

    form.insert("sourceCode", &code);

    let require_addr_selector: Selector =
        Selector::parse(r#"form[class="form-horizontal form-code-submit"]"#).unwrap();

    let require_addr: &str = html
        .select(&require_addr_selector)
        .next()
        .unwrap()
        .attr("action")
        .unwrap();

    let addr: String = String::from("https://atcoder.jp") + require_addr;

    client.post(&addr).form(&form).send().await.unwrap();

    println!("{}", Marker::plus("\x1b[32mFinished successfully\x1b[m"));
}
