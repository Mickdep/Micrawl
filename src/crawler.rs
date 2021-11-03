use crate::{config::ArgCollection, crawl_reporter};
use reqwest::StatusCode;
use select::{document::Document, predicate::Name};
use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::Instant,
};
use url::Url;

pub struct Crawler {
    queue: Vec<CrawlQueueItem>,
    crawled_pages: Vec<CrawlResult>,
    block_list: Vec<Url>,
    config: ArgCollection,
    start_time: Instant,
    robots_content: Option<String>
}

#[derive(Clone)]
pub struct CrawlResult {
    pub url: Url,
    pub status_code: Option<StatusCode>,
}

struct CrawlQueueItem {
    url: Url,
    is_external: bool,
}

struct ThreadCrawlResult {
    url: Url,
    result: Result<reqwest::blocking::Response, reqwest::Error>,
}

impl Crawler {
    pub fn new(arg_collection: ArgCollection) -> Crawler {
        let mut crawler = Crawler {
            queue: Vec::new(),
            crawled_pages: Vec::new(),
            block_list: Vec::new(),
            config: arg_collection,
            start_time: Instant::now(),
            robots_content: None
        };

        //Add initial url to the queue.
        let crawl_queue_item = CrawlQueueItem {
            url: Url::parse(crawler.config.host.as_str()).unwrap(),
            is_external: false,
        };
        crawler.queue.push(crawl_queue_item);

        return crawler;
    }

    pub fn crawl(&mut self) {
        //If we need to extract robots content; do so and print it.
        if self.config.extract_robots_content {
            let mut base_clone = self.config.host.as_str().to_owned();
            base_clone.push_str("/robots.txt");
            if let Some(robots) = self.get_robots_content(Url::parse(&base_clone).unwrap()) {
                self.print_robots_content(&robots);
                self.robots_content = Some(robots);
            }
        }

        let (tx, rx) = mpsc::channel(); //Create sending an receiving channel for communication between threads.
        loop {
            if self.queue.is_empty() {
                break;
            }
            let mut workers: Vec<JoinHandle<()>> = Vec::new();

            // Create all worker threads in the loop below.
            while workers.len() < self.config.threads.into() {
                if let Some(current) = self.queue.pop() {
                    if current.is_external {
                        //If url is external we just print the result and add it to the crawled list.
                        self.print_result("...", &current.url.as_str());
                        let crawl_result = CrawlResult {
                            status_code: None,
                            url: current.url,
                        };
                        self.crawled_pages.push(crawl_result);
                    } else if self.should_crawl(&current.url) {
                        let thread_tx = tx.clone();
                        //Spawn thread that executes a GET request to the dequeued URL.
                        let worker = self.create_worker(current, thread_tx);
                        workers.push(worker);
                    }
                } else {
                    break;
                }
            }

            //Receive the results from all workers and process these.
            for _ in &workers {
                self.process_worker(&rx);
            }

            //Wait for all workers to finish.
            for worker in workers {
                if let Err(_) = worker.join() {
                    eprintln!("Error occurred in thread.");
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
                elapsed_ms: self.start_time.elapsed().subsec_millis()
            };
            crawl_reporter::report(report_info);
        }
    }

    fn create_worker(
        &self,
        current: CrawlQueueItem,
        thread_tx: Sender<ThreadCrawlResult>,
    ) -> JoinHandle<()> {
        let worker = thread::spawn(move || {
            let http_client = reqwest::blocking::Client::new();
            let result = http_client
                .get(current.url.clone())
                .header("User-Agent", randua::new().to_string())
                .send();

            let thread_result = ThreadCrawlResult {
                url: current.url,
                result,
            };

            //Send this result over the mpsc channel
            if let Err(_) = thread_tx.send(thread_result) {
                eprintln!("Encountered an error in thread.");
            }
        });

        return worker;
    }

    fn process_worker(&mut self, rx: &Receiver<ThreadCrawlResult>) {
        if let Ok(recv) = rx.recv() {
            match recv.result {
                Ok(result) => {
                    let crawl_result = CrawlResult {
                        status_code: Some(result.status()),
                        url: recv.url.clone(),
                    };

                    self.print_result(
                        crawl_result.status_code.unwrap().as_str(),
                        crawl_result.url.as_str(),
                    );

                    let from = result.url().clone(); //Clone here because Document::from_read() takes ownership of this object.
                    self.crawled_pages.push(crawl_result); //Register this URL as crawled by adding it to the vector.
                    if result.status().is_success() {
                        if self.is_same_domain(result.url()) || self.is_same_host(result.url()) {
                            if let Ok(doc) = Document::from_read(result) {
                                self.extract_anchor_hrefs(&doc, &from);
                                self.extract_form_actions(&doc, &from);
                            }
                        }
                    }
                }
                Err(result) => {
                    if let Some(url) = result.url() {
                        let mut reason = "";
                        if result.is_connect() {
                            reason = "Can't connect";
                        }
                        if result.is_redirect() {
                            reason = "Redirect policy";
                        }
                        if result.is_timeout() {
                            reason = "Timeout";
                        }
                        eprintln!("[!] Error with request to URL: {}. ({})", url, reason);
                        self.block_list.push(url.clone());
                    }
                }
            }
            // Do something with the results here
        } else {
            eprintln!("Error occurred in thread.");
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
                            || self.config.list_external
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

    fn extract_images(&mut self, doc: &Document) {
        
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

    pub fn get_robots_content(&self, url: Url) -> Option<String> {
        let http_client = reqwest::blocking::Client::new();
        let result = http_client
            .get(url)
            .header("User-Agent", randua::new().to_string())
            .send();
        if let Ok(res) = result {
            if res.status().is_success() {
                if let Ok(res_text) = res.text() {
                    return Some(res_text.trim_end().to_string());
                } else {
                    eprintln!("[!] Robots.txt exists but could not extract content. Please manually extract content.");
                }
            } else {
                eprintln!("[!] Could not find the robots.txt file in the default location.");
            }
        }

        return None;
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
