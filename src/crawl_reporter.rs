use reqwest::Url;
use std::{fs, io::Write};

use crate::{config::ArgCollection, crawler::{CrawlResult, UrlType}};

#[derive(Clone)]
pub struct ReportInfo {
    pub discovered_links: Vec<CrawlResult>,
    pub config: ArgCollection,
    pub robots: Option<String>,
    pub elapsed_secs: u64,
    pub elapsed_ms: u32,
}

pub fn report(report_info: ReportInfo) {
    //Create the file
    if let Ok(mut file) = fs::File::create(&report_info.config.file) {
        let mut output = String::from(format!(
            "[Micrawl report for {}] \n\n",
            report_info.config.host
        ));

        //Append the config
        output.push_str(&report_info.config.as_string());

        //Append the robots.txt content if present
        if let Some(content) = &report_info.robots {
            output.push_str(&format!(
                "=========== Robots.txt ===========\n{}\n==================================\n\n",
                content
            ));
        }

        //Append all the crawled pages
        for crawl_result in &report_info.discovered_links {
            let mut prepend = "🔗";
            if crawl_result.url_type == UrlType::External {
                prepend = "↗";
            }else if crawl_result.url_type == UrlType::Form {
                prepend = "📝";
            }
            output.push_str(&format!("{} {} \n", prepend, crawl_result.url));
        }

        //Append the final info (amount of crawled pages and the elapsed time)
        output.push_str(&format!(
            "\nFound {} links in {}.{} sec.",
            report_info.discovered_links.len(),
            report_info.elapsed_secs,
            report_info.elapsed_ms
        ));

        //Show error if file can't be written
        if let Err(_) = file.write_all(output.as_bytes()) {
            eprintln!("[!] Failed writing output to file.");
        }
    } else {
        eprintln!("[!] Failed to create output file.");
    }
}
