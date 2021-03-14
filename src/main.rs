//- Begin of module tree definition -
mod config;
mod crawler;
//- End of module tree definition -

use clap::{self, App, Arg};
use config::ArgCollection;
use std::{process::exit};

fn main() {
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
            .get_matches();

    match ArgCollection::parse(matches) {
        Ok(arg_collection) => {
            match arg_collection.validate() {
                Ok(_) => {
                    let file_clone = arg_collection.file.clone(); //Clone here because we give Crawler ownership of the arg_collection.
                    let mut crawler = crawler::Crawler::new(arg_collection); //Needs to be mutable because the crawl function changes its internal state.
                    crawler.print_config();
                    crawler.crawl();
                    crawler.print_stats();
                    if file_clone.as_os_str().len() > 0 {
                        if let Err(report_err) = crawler.report() {
                            terminate(report_err);
                        }
                    }
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
