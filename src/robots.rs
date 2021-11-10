use std::time::Duration;

use reqwest::Url;

pub fn try_extract(base_url: &Url) -> Option<String> {
    if let Ok(url) = base_url.join("robots.txt") {
        return get_robots_content(url);
    }

    return None;
}

fn get_robots_content(url: Url) -> Option<String> {
    let http_client = reqwest::blocking::Client::new();
    let result = http_client
        .get(url)
        .header("User-Agent", randua::new().to_string())
        .timeout(Duration::from_secs(30))
        .send();
    if let Ok(res) = result {
        if res.status().is_success() {
            if let Ok(res_text) = res.text() {
                return Some(res_text.trim_end().to_string());
            } else {
                eprintln!("[!] Robots.txt exists but could not extract content. Please manually extract content.");
            }
        } else {
            eprintln!("[!] Robots.txt not found at the default location.");
        }
    }

    return None;
}
