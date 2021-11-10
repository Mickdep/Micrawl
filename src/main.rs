//- Begin of module tree definition -
mod config;
mod crawl_reporter;
mod crawler;
mod robots;
//- End of module tree definition -

use clap::{self, App, Arg};
use config::ArgCollection;
use std::process::exit;

#[tokio::main]
async fn main() {
    print_banner();
    let matches = App::new("Micrawl")
        .arg(Arg::with_name("url")
            .short("u")
            .long("url")
            .value_name("url")
            .help("Specifies the host to crawl. Requires protocol prefix (http(s)://<ip> || http(s)://<url>).")
            .takes_value(true)
            .required(true))
        .arg(Arg::with_name("output_file")
            .short("o")
            .long("output")
            .value_name("output_file")
            .help("Specifies the file to write the output to. Saves this file in the directory this is executing in.")
            .takes_value(true)
            .required(false))
        .arg(Arg::with_name("list_external")
            .short("e")
            .long("external")
            .value_name("list_external")
            .help("Additionally look for external pointing links.")
            .takes_value(false)
            .required(false))
        .arg(Arg::with_name("extract_robots_content")
            .short("r")
            .long("robots")
            .value_name("extract_robots_content")
            .help("Extract content from robots.txt.")
            .takes_value(false)
            .required(false))
            .get_matches();

    match ArgCollection::parse(matches) {
        //Parse the arguments provided
        Ok(arg_collection) => {
            match arg_collection.validate() {
                //Validate the arguments
                Ok(_) => {
                    arg_collection.print(); //Show the config being used
                    let mut crawler = crawler::Crawler::new(arg_collection); //Needs to be mutable because the crawl function changes its internal state.
                    crawler.crawl().await; //Crawl
                }
                Err(arg_validation_err) => {
                    terminate(arg_validation_err);
                }
            }
        }
        Err(arg_parser_err) => {
            terminate(arg_parser_err);
        }
    }
}

fn print_banner() {
    println!("   _____  .__                           .__   ");
    println!("  /     \\ |__| ________________ __  _  _|  |       ||   ||");
    println!(" /  \\ /  \\|  |/ ___\\_  __ \\__  \\\\ \\/\\/  /  |        \\\\()//");
    println!("/    Y    \\  \\  \\___|  | \\// __ \\\\     /|  |__     //(__)\\\\");
    println!("\\____|__  /__|\\___  >__|  (____  /\\/\\_/ |____/     ||    ||");
    println!("        \\/        \\/           \\/    ");
}

fn terminate(err: &str) {
    eprintln!("[!] {}. Terminating.", err);
    exit(1);
}
