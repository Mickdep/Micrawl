use crate::{config::ArgCollection, crawl_reporter, robots};
use futures::stream::FuturesUnordered;
use reqwest::{Error, Response, StatusCode};
use select::{document::Document, predicate::Name};
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;
use url::Url;

pub struct Crawler {
    queue: Vec<CrawlQueueEntry>,
    crawled_pages: Vec<CrawlResult>,
    block_list: Vec<Url>,
    found_links: Vec<Url>,
    config: ArgCollection,
    start_time: Instant,
    robots_content: Option<String>,
}

#[derive(Clone)]
pub struct CrawlResult {
    pub url: Url,
    pub status_code: Option<StatusCode>,
}

#[derive(Clone)]
struct CrawlQueueEntry {
    url: Url,
    is_external: bool,
}

impl Crawler {
    pub fn new(arg_collection: ArgCollection) -> Crawler {
        let mut crawler = Crawler {
            queue: Vec::new(),
            crawled_pages: Vec::new(),
            block_list: Vec::new(),
            found_links: Vec::new(),
            config: arg_collection,
            start_time: Instant::now(),
            robots_content: None,
        };

        //Add initial url to the queue.
        let crawl_queue_item = CrawlQueueEntry {
            url: Url::parse(crawler.config.host.as_str()).unwrap(),
            is_external: false,
        };
        crawler.queue.push(crawl_queue_item);

        return crawler;
    }

    pub async fn crawl(&mut self) {
        if self.config.extract_robots_content {
            if let Some(robots) = robots::try_extract(&self.config.host) {
                self.print_robots_content(&robots);
                self.robots_content = Some(robots);
            }
        }

        let client = reqwest::Client::new(); //Create single Client and clone that so we make use of the connection pool. https://docs.rs/reqwest/0.10.9/reqwest/struct.Client.html

        loop {
            if self.queue.is_empty() {
                break;
            }

            let tasks = FuturesUnordered::new();
            while let Some(current) = self.queue.pop() {
                println!("Adding task for url {}", &current.url);
                let client_clone = client.clone();
                let handle: JoinHandle<Result<Response, Error>> = tokio::spawn(async move {
                    let result = client_clone
                        .get(current.url)
                        .header("User-Agent", randua::new().to_string())
                        .send()
                        .await;
                    return result;
                });
                tasks.push(handle);
            }

            if tasks.len() < 1 {
                break;
            }

            // await all tasks here.
            let results = futures::future::join_all(tasks).await;
            for result in results {
                if let Ok(unwrapped) = result {
                    if let Ok(response) = unwrapped {
                        let crawl_result = CrawlResult {
                            status_code: Some(response.status()),
                            url: response.url().clone(),
                        };

                        self.crawled_pages.push(crawl_result); //Register this URL as crawled by adding it to the vector.

                        let from_url = response.url().clone(); //Clone here because response.text() consumes the object.
                        if response.status().is_success() {
                            if let Ok(text) = response.text().await {
                                let doc = Document::from(text.as_str());

                                //Extract all anchor hrefs and try to join them with the url that the current request was done to
                                let anchor_hrefs = self.extract_anchor_hrefs(&doc);
                                for str in anchor_hrefs {
                                    if let Ok(url) = Url::parse(&str){
                                        if !self.found_links.contains(&url) {
                                            self.found_links.push(url);
                                        }
                                    }
                                }

                                //Extract all form actions and try to join them with the url that the current request was done to
                                let form_actions = self
                                    .extract_form_actions(&doc)
                                    .iter()
                                    .filter_map(|x| Url::parse(x).ok())
                                    .collect();

                                //Print all results
                                self.print_findings(form_actions);
                                self.print_findings(anchor_hrefs);

                                //Now...Here we check which other pages we are going to add to the queue and/or going to crawl.
                                anchor_hrefs.iter().for_each(|x| {
                                    if self.should_crawl(x) {
                                        self.queue.push(value)
                                    }
                                });
                            }
                        }
                    }
                }
            }
        }
        self.print_stats();
        if self.config.file.as_os_str().len() > 0 {
            //We could use lifetimes here instead of cloning.
            let report_info = crawl_reporter::ReportInfo {
                crawled_pages: self.crawled_pages.clone(),
                config: self.config.clone(),
                robots: self.robots_content.clone(),
                elapsed_secs: self.start_time.elapsed().as_secs(),
                elapsed_ms: self.start_time.elapsed().subsec_millis(),
            };
            crawl_reporter::report(report_info);
        }
    }

    fn extract_anchor_hrefs(&mut self, doc: &Document) -> Vec<String> {
        let mut results = Vec::new();
        doc.find(Name("a")) //Find all anchor tags
            .filter_map(|x| x.attr("href")) //Filter map to only contain the href values
            .for_each(|y| {
                results.push(String::from(y));
            });
        return results;
    }

    fn extract_form_actions(&mut self, doc: &Document) -> Vec<String> {
        let mut results = Vec::new();
        doc.find(Name("form")) //Find all form tags
            .filter_map(|x| x.attr("action")) //Filter map to only contain the action values
            .for_each(|y| {
                results.push(String::from(y));
            });
        return results;
    }

    fn should_crawl(&mut self, url: &Url) -> bool {
        return !self.already_crawled(url)
            && !self.is_in_queue(url)
            && !self.is_in_blocklist(url)
            && self.is_webpage(url);
    }

    fn is_in_blocklist(&self, url: &Url) -> bool {
        return self
            .block_list
            .iter()
            .any(|elem| elem.as_str() == url.as_str());
    }

    fn is_webpage(&self, url: &Url) -> bool {
        if let Some(segments) = url.path_segments() {
            if let Some(last) = segments.last() {
                if last.contains(".") {
                    let last_split: Vec<&str> = last.split('.').collect();
                    if last_split[1] != "html" && last_split[1] != "php" {
                        return false;
                    }
                }
            }
        }
        return true;
    }

    fn is_in_queue(&self, url: &Url) -> bool {
        return self
            .queue
            .iter()
            .any(|elem| elem.url.as_str() == url.as_str());
    }

    fn already_crawled(&self, url: &Url) -> bool {
        return self
            .crawled_pages
            .iter()
            .any(|elem| elem.url.as_str() == url.as_str());
    }

    fn is_external(&self, url: &Url) -> bool {
        return !self.is_same_domain(url) && !self.is_same_host(url);
    }

    fn is_same_domain(&self, url: &Url) -> bool {
        if let Some(base_domain) = self.config.host.domain() {
            if let Some(domain) = url.domain() {
                if domain.contains(base_domain) {
                    return true;
                }
            }
        }
        return false;
    }

    fn is_same_host(&self, url: &Url) -> bool {
        if let Some(base_host) = self.config.host.host_str() {
            if let Some(host) = url.host_str() {
                if base_host == host {
                    return true;
                }
            }
        }
        return false;
    }

    pub fn print_findings(&self, findings: Vec<Url>) {
        for url in findings {
            println!("[+] {}", url);
        }
    }

    pub fn print_stats(&self) {
        let elapsed = self.start_time.elapsed().as_secs();
        let elapsed_ms = self.start_time.elapsed().subsec_millis();
        println!(
            "\nFound {} links in {}.{} sec.",
            self.crawled_pages.len(),
            elapsed,
            elapsed_ms
        );
    }

    pub fn print_robots_content(&self, robots: &str) {
        println!("");
        println!("=========== Robots.txt ==========");
        println!("{}", robots);
        println!("=================================");
        println!("");
    }

    pub fn print_result(&self, status: &str, url: &str) {
        println!("[+] [{}]: {}", status, url);
    }
}
