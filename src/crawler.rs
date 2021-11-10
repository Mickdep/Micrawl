use crate::{config::ArgCollection, crawl_reporter, robots};
use futures::stream::FuturesUnordered;
use reqwest::{Error, Response};
use select::{document::Document, predicate::Name};
use std::time::Instant;
use tokio::task::JoinHandle;
use url::Url;

#[derive(PartialEq, Clone)]
pub enum UrlType {
    Link,
    Form,
    External
}
pub struct Crawler {
    queue: Vec<Url>,
    crawled_pages: Vec<Url>,
    block_list: Vec<Url>,
    discovered_links: Vec<CrawlResult>,
    config: ArgCollection,
    start_time: Instant,
    robots_content: Option<String>,
}

#[derive(PartialEq, Clone)]
pub struct CrawlResult {
    pub url: Url,
    pub url_type: UrlType
}

impl Crawler {
    pub fn new(arg_collection: ArgCollection) -> Crawler {
        let mut crawler = Crawler {
            queue: Vec::new(),
            crawled_pages: Vec::new(),
            block_list: Vec::new(),
            discovered_links: Vec::new(),
            config: arg_collection,
            start_time: Instant::now(),
            robots_content: None,
        };

        //Add initial url to the queue.
        crawler
            .queue
            .push(Url::parse(crawler.config.host.as_str()).unwrap());

        return crawler;
    }

    pub async fn crawl(&mut self) {
        if self.config.extract_robots_content {
            if let Some(robots) = robots::try_extract(&self.config.host) {
                self.print_robots_content(&robots);
                self.robots_content = Some(robots);
            }
        }

        //Don't want to match on Ok or Error here. Just panic if no client can be constructed.
        // let client = reqwest::ClientBuilder::new()
        //     .redirect(Policy::none())
        //     .build().unwrap();
        
        let client = reqwest::Client::new(); //Create single Client and clone that so we make use of the connection pool. https://docs.rs/reqwest/0.10.9/reqwest/struct.Client.html
        loop {
            if self.queue.is_empty() {
                break;
            }

            let tasks = FuturesUnordered::new();
            while let Some(current) = self.queue.pop() {
                let client_clone = client.clone();
                self.crawled_pages.push(current.clone());
                let handle: JoinHandle<Result<Response, Error>> = tokio::spawn(async move {
                    let result = client_clone
                        .get(current)
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
                        // self.crawled_pages.push(response.url().clone()); //Register this URL as crawled by adding it to the list.

                        let from_url = response.url().clone(); //Clone here because response.text() consumes the object.
                        if response.status().is_success() {
                            if let Ok(text) = response.text().await {
                                let doc = Document::from(text.as_str());

                                let anchor_hrefs = self.extract_anchor_hrefs(&doc, &from_url);
                                for url in anchor_hrefs {
                                    if self.should_print(&url) {
                                        if self.is_external(&url) {
                                            if self.config.list_external {
                                                self.print_finding("↗", &url);
                                            }
                                        } else {
                                            self.print_finding("🔗", &url);
                                        }
                                    }

                                    if self.should_enqueue(&url) {
                                        self.queue.push(url.clone());
                                    }

 

                                    if !self.discovered_links.iter().any(|elem| &elem.url == &url) {
                                        let mut crawl_result = CrawlResult {
                                            url,
                                            url_type: UrlType::Link
                                        };
                                        if self.is_external(&crawl_result.url) {
                                            if self.config.list_external {
                                                crawl_result.url_type = UrlType::External;
                                                self.discovered_links.push(crawl_result);
                                            }
                                        } else {
                                            self.discovered_links.push(crawl_result);
                                        }
                                    }

                                }

                                let form_actions = self.extract_form_actions(&doc, &from_url);
                                for url in form_actions {
                                    if self.should_print(&url) {
                                        self.print_finding("📝", &url);
                                        if !self.discovered_links.iter().any(|elem| &elem.url == &url) {
                                            let crawl_result = CrawlResult {
                                                url,
                                                url_type: UrlType::Form
                                            };
                                            self.discovered_links.push(crawl_result);
                                        }
                                    }
                                }
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
                discovered_links: self.discovered_links.clone(),
                config: self.config.clone(),
                robots: self.robots_content.clone(),
                elapsed_secs: self.start_time.elapsed().as_secs(),
                elapsed_ms: self.start_time.elapsed().subsec_millis(),
            };
            crawl_reporter::report(report_info);
        }
    }

    fn extract_anchor_hrefs(&mut self, doc: &Document, from: &Url) -> Vec<Url> {
        let mut results = Vec::new();
        doc.find(Name("a")) //Find all anchor tags
            .filter_map(|x| x.attr("href")) //Filter map to only contain the href values
            .for_each(|y| {
                if let Ok(url) = from.join(y) {
                    results.push(url);
                }
            });
        return results;
    }

    fn extract_form_actions(&mut self, doc: &Document, from: &Url) -> Vec<Url> {
        let mut results = Vec::new();
        doc.find(Name("form")) //Find all form tags
            .filter_map(|x| x.attr("action")) //Filter map to only contain the action values
            .for_each(|y| {
                if let Ok(url) = from.join(y) {
                    results.push(url);
                }
            });
        return results;
    }

    fn should_enqueue(&mut self, url: &Url) -> bool {
        return !self.already_crawled(url)
            && !self.is_in_queue(url)
            && !self.is_in_blocklist(url)
            && !self.is_external(url)
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
        return self.queue.iter().any(|elem| elem.as_str() == url.as_str());
    }

    fn already_crawled(&self, url: &Url) -> bool {
        return self
            .crawled_pages
            .iter()
            .any(|elem| elem.as_str() == url.as_str());
    }

    fn is_external(&self, url: &Url) -> bool {
        return !self.is_same_domain(url) && !self.is_same_host(url);
    }

    fn should_print(&self, url: &Url) -> bool {
        return !self.discovered_links.iter().any(|elem| &elem.url == url);
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

    pub fn print_finding(&self, prepend: &str, finding: &Url) {
        println!("{} {}", prepend, finding);
    }

    pub fn print_stats(&self) {
        let elapsed = self.start_time.elapsed().as_secs();
        let elapsed_ms = self.start_time.elapsed().subsec_millis();
        println!(
            "\nFound {} links in {}.{} sec.",
            self.discovered_links.len(),
            elapsed,
            elapsed_ms
        );
    }

    pub fn print_robots_content(&self, robots: &str) {
        println!("=========== Robots.txt ===========");
        println!("{}", robots);
        println!("==================================");
        println!("");
    }
}
