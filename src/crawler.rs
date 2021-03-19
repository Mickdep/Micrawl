use crate::config::ArgCollection;
use reqwest::StatusCode;
use select::{document::Document, predicate::Name};
use std::{fs, io::Write, path::PathBuf, time::Instant};
use url::Url;

pub struct Crawler {
    queue: Vec<CrawlQueueItem>,
    crawled_pages: Vec<CrawlResult>,
    block_list: Vec<Url>,
    base: Url,
    file: PathBuf,
    start_time: Instant,
    list_external: bool,
}

pub struct CrawlResult {
    url: Url,
    status_code: Option<StatusCode>,
}

pub struct CrawlQueueItem {
    url: Url,
    is_external: bool,
}

impl Crawler {
    pub fn new(arg_collection: ArgCollection) -> Crawler {
        let mut crawler = Crawler {
            queue: Vec::new(),
            crawled_pages: Vec::new(),
            block_list: Vec::new(),
            base: arg_collection.host,
            file: arg_collection.file,
            start_time: Instant::now(),
            list_external: arg_collection.list_external,
        };

        let crawl_queue_item = CrawlQueueItem {
            url: Url::parse(crawler.base.as_str()).unwrap(),
            is_external: false,
        };

        crawler.queue.push(crawl_queue_item);

        return crawler;
    }

    pub fn crawl(&mut self) {
        let http_client = reqwest::blocking::Client::new();

        while let Some(current) = self.queue.pop() {
            if !current.is_external {
                //Only make the request if URL is not external
                match http_client.get(current.url.clone()).send() {
                    Ok(result) => {
                        let crawl_result = CrawlResult {
                            status_code: Some(result.status()),
                            url: current.url.clone(),
                        };

                        self.print_result(result.status().as_str(), &current.url.as_str());
                        let from = result.url().clone(); //Clone here because Document::from_read() takes ownership of this object.
                        self.crawled_pages.push(crawl_result);
                        if result.status().is_success() {
                            if self.is_same_domain(result.url()) || self.is_same_host(result.url())
                            {
                                if let Ok(doc) = Document::from_read(result) {
                                    self.extract_anchor_hrefs(&doc, &from);
                                    self.extract_form_actions(&doc, &from);
                                }
                            }
                        }
                    }
                    Err(result) => {
                        if let Some(url) = result.url() {
                            eprintln!("[!] Can't reach URL: {}", url);
                            self.block_list.push(url.clone());
                        }
                    }
                }
            } else {
                //If url is external we just print the result and at it to the crawled list.
                self.print_result("...", &current.url.as_str());
                let crawl_result = CrawlResult {
                    status_code: None,
                    url: current.url,
                };
                self.crawled_pages.push(crawl_result);
            }
        }
    }

    fn extract_anchor_hrefs(&mut self, doc: &Document, from: &Url) {
        doc.find(Name("a")) //Find all anchor tags
            .filter_map(|x| x.attr("href")) //Filter map to only contain the href values
            .for_each(|y| {
                if let Ok(url) = from.join(y) {
                    if self.should_crawl(&url) {
                        if self.is_same_domain(&url)
                            || self.is_same_host(&url)
                            || self.list_external
                        {
                            let crawl_queue_item = CrawlQueueItem {
                                is_external: !self.is_same_domain(&url) && !self.is_same_host(&url),
                                url,
                            };
                            self.queue.push(crawl_queue_item);
                        }
                    }
                }
            });
    }

    fn extract_form_actions(&mut self, doc: &Document, from: &Url) {
        doc.find(Name("form")) //Find all form tags
            .filter_map(|x| x.attr("action")) //Filter map to only contain the action values
            .for_each(|y| {
                if let Ok(url) = from.join(y) {
                    if !self.crawled_pages_contains(&url) {
                        self.print_result("...", url.as_str());
                        let crawl_result = CrawlResult {
                            status_code: None,
                            url,
                        };
                        self.crawled_pages.push(crawl_result);
                    }
                }
            });
    }

    fn should_crawl(&mut self, url: &Url) -> bool {
        if self
            .block_list
            .iter()
            .any(|elem| elem.as_str() == url.as_str())
        {
            return false;
        }
        //Check if the url has already been crawled
        if !self.crawled_pages_contains(url) {
            //Make sure the queue doesn't already contain this.
            if !self.queue_contains(url) {
                return true;
            }
        }

        return false;
    }

    fn queue_contains(&self, url: &Url) -> bool {
        return self
            .queue
            .iter()
            .any(|elem| elem.url.as_str() == url.as_str());
    }

    fn crawled_pages_contains(&self, url: &Url) -> bool {
        return self
            .crawled_pages
            .iter()
            .any(|elem| elem.url.as_str() == url.as_str());
    }

    fn is_same_domain(&self, url: &Url) -> bool {
        if let Some(base_domain) = self.base.domain() {
            if let Some(domain) = url.domain() {
                if domain.contains(base_domain) {
                    return true;
                }
            }
        }
        return false;
    }

    fn is_same_host(&self, url: &Url) -> bool {
        if let Some(base_host) = self.base.host_str() {
            if let Some(host) = url.host_str() {
                if base_host == host {
                    return true;
                }
            }
        }
        return false;
    }

    pub fn print_config(&self) {
        println!("[~] Crawling URL: {}", self.base);
        if self.file.as_os_str().len() > 0 {
            println!("[~] Writing output to file: {}", self.file.display());
        }
        println!("");
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

    pub fn report(&self) -> Result<(), &'static str> {
        //Check whether an output file was parsed/specified.
        if let Ok(mut file) = fs::File::create(&self.file) {
            let mut output = String::from(format!("[Micrawl report for {}] \n\n", self.base));
            for result in &self.crawled_pages {
                if let Some(status) = result.status_code {
                    output.push_str(format!("[{}] {} \n", status.as_str(), result.url).as_str());
                } else {
                    output.push_str(format!("[...] {} \n", result.url).as_str());
                }
            }

            let elapsed = self.start_time.elapsed().as_secs();
            let elapsed_ms = self.start_time.elapsed().subsec_millis();
            output.push_str(
                format!(
                    "\nFound {} links in {}.{} sec.",
                    self.crawled_pages.len(),
                    elapsed,
                    elapsed_ms
                )
                .as_str(),
            );
            if let Err(_) = file.write_all(output.as_bytes()) {
                return Err("Failed writing output to file");
            }
        } else {
            return Err("Failed to create output file");
        }

        return Ok(());
    }

    pub fn print_result(&self, status: &str, url: &str) {
        println!("[+] [{}]: {}", status, url);
    }
}
