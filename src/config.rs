use clap::ArgMatches;
use reqwest::Url;
use std::{env, path::PathBuf};

pub struct ArgCollection {
    pub host: Url,
    pub file: PathBuf,
    pub list_external: bool,
}

impl ArgCollection {
    pub fn parse(arg_matches: ArgMatches) -> Result<ArgCollection, &'static str> {
        //If not even the url argument is present we terminate immediately.
        //Create struct to store values in. Initialised with temporary values.
        let mut arg_collection = ArgCollection {
            host: Url::parse("http://127.0.0.1").unwrap(), //Default vlaue
            file: PathBuf::new(),
            list_external: false,
        };

        if let Some(url) = arg_matches.value_of("url") {
            if let Ok(parsed_url) = Url::parse(url) {
                arg_collection.host = parsed_url;
            }else{
                return Err("Failed to parse host. Please check if you've specified the protocol prefix");
            }
        }
        
        if let Some(output_file) = arg_matches.value_of("output_file"){
            if let Ok(mut path) = env::current_exe(){
                path.set_file_name(output_file);
                arg_collection.file = path;
            }else{
                return Err("Could not resolve valid file path");
            }
        }

        if arg_matches.is_present("list_external"){
            arg_collection.list_external = true;
        }

        return Ok(arg_collection);
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if let Err(_) = reqwest::blocking::get(self.host.as_str()) {
            return Err("Failed to connect to host");
        }
        return Ok(());
    }
}
