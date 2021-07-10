use reqwest::Url;
use std::{fs, io::Write, path::PathBuf, time::Instant};
use crate::crawler::CrawlResult;

pub fn report(start_time: Instant, crawled_pages: &Vec<CrawlResult>, file: &PathBuf, base: &Url) {
    //Check whether an output file was parsed/specified.
    let elapsed = start_time.elapsed().as_secs();
    let elapsed_ms = start_time.elapsed().subsec_millis();
    if let Ok(mut file) = fs::File::create(file) {
        let mut output = String::from(format!("[Micrawl report for {}] \n\n", base));
        for result in crawled_pages {
            if let Some(status) = result.status_code {
                output.push_str(format!("[{}] {} \n", status.as_str(), result.url).as_str());
            } else {
                output.push_str(format!("[...] {} \n", result.url).as_str());
            }
        }
        output.push_str(
            format!(
                "\nFound {} links in {}.{} sec.",
                crawled_pages.len(),
                elapsed,
                elapsed_ms
            )
            .as_str(),
        );
        if let Err(_) = file.write_all(output.as_bytes()) {
            eprintln!("[!] Failed writing output to file.");
        }
    } else {
        eprintln!("[!] Failed to create output file.");
    }
}
