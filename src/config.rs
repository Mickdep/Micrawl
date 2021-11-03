use clap::ArgMatches;
use reqwest::Url;
use std::{env, fs, path::PathBuf};

#[derive(Clone)]
pub struct ArgCollection {
    pub host: Url,
    pub file: PathBuf,
    pub list_external: bool,
    pub extract_robots_content: bool,
    pub threads: u8,
    pub extract_images: bool,
    should_report_to_file: bool,
    max_threads: u8,
}

impl Default for ArgCollection {
    fn default() -> Self {
        ArgCollection {
            host: Url::parse("http://127.0.0.1").unwrap(), //Default vlaue
            file: PathBuf::new(),
            list_external: false,
            extract_robots_content: false,
            threads: 10,
            extract_images: false,
            should_report_to_file: false,
            max_threads: 30,
        }
    }
}

impl ArgCollection {
    pub fn parse(arg_matches: ArgMatches) -> Result<ArgCollection, &'static str> {
        let mut arg_collection = ArgCollection::default();

        if let Some(url) = arg_matches.value_of("url") {
            if let Ok(parsed_url) = Url::parse(url) {
                arg_collection.host = parsed_url;
            } else {
                return Err(
                    "Failed to parse host. Please check if you've specified the protocol prefix",
                );
            }
        }

        if let Some(output_file) = arg_matches.value_of("output_file") {
            arg_collection.should_report_to_file = true;
            if let Ok(mut path) = env::current_dir() {
                path.push(output_file);
                arg_collection.file = path;
            } else {
                return Err("Could not resolve valid file path");
            }
        }

        if arg_matches.is_present("list_external") {
            arg_collection.list_external = true;
        }

        if arg_matches.is_present("extract_robots_content") {
            arg_collection.extract_robots_content = true;
        }

        if let Some(threads) = arg_matches.value_of("threads") {
            if let Ok(res) = threads.parse::<u8>() {
                arg_collection.threads = res;
            } else {
                return Err("Could not parse the amount of threads");
            }
        }

        if arg_matches.is_present("extract_images"){
            arg_collection.extract_images = true;
        }

        return Ok(arg_collection);
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if let Err(_) = reqwest::blocking::get(self.host.as_str()) {
            return Err("Failed to connect to host");
        }

        if self.should_report_to_file {
            if let Err(_) = fs::File::create(&self.file) {
                return Err("Failed to create output file");
            }
        }

        if self.threads > self.max_threads {
            return Err("Can't run with more than 30 threads");
        } else if self.threads < 1 {
            return Err("Can't runt with less than 1 thread");
        }

        return Ok(());
    }

    pub fn print(&self) {
        println!("[~] Crawling URL: {}", self.host);
        println!("[~] Running with {} threads", self.threads);

        if self.file.as_os_str().len() > 0 {
            println!("[~] Writing output to file: {}", self.file.display());
        }
        if self.list_external {
            println!("[~] Listing external links");
        }
        if self.extract_robots_content {
            println!("[~] Extracting robots.txt content");
        }

        if self.extract_images {
            println!("[~] Extracting images");
        }
    }

    pub fn as_string(&self) -> String {
        let mut output = String::from(format!("[~] Crawling URL: {}\n", self.host));
        output.push_str(format!("[~] Running with {} threads\n", self.threads).as_str());

        if self.file.as_os_str().len() > 0 {
            output.push_str(format!("[~] Writing output to file: {}\n", self.file.display()).as_str());
        }
        if self.list_external {
            output.push_str(format!("[~] Listing external links\n").as_str());

        }
        if self.extract_robots_content {
            output.push_str(format!("[~] Extracting robots.txt content\n").as_str());
        }
        if self.extract_images {
            output.push_str(format!("[~] Extracting images\n").as_str());
        }
        output.push_str("\n");
        return output;
    }
}
