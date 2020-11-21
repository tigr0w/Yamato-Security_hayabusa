extern crate csv;
extern crate chrono;

use crate::detections::rule;
use crate::detections::rule::RuleNode;
use crate::detections::print::{Message};
use crate::yaml::ParseYaml;

use chrono::{TimeZone, Utc};
use evtx::EvtxParser;
use serde_json::{Error, Value};

const DIRPATH_RULES: &str = "rules";

#[derive(Debug)]
pub struct Detection {
    
}

impl Detection {
    pub fn new() -> Detection {
        Detection {
        }
    }

    pub fn start(&mut self, mut parser: EvtxParser<std::fs::File>) {
        // from .etvx to json
        let event_records: Vec<Value> = parser
            .records_json()
            .filter_map(|result_record| {
                if result_record.is_err() {
                    eprintln!("{}", result_record.unwrap_err());
                    return Option::None;
                }

                //// refer https://rust-lang-nursery.github.io/rust-cookbook/encoding/complex.html
                let result_json: Result<Value, Error> =
                    serde_json::from_str(&result_record.unwrap().data);
                if result_json.is_err() {
                    eprintln!("{}", result_json.unwrap_err());
                    return Option::None;
                }
                return result_json.ok();
            })
            .collect();

        event_records.iter().for_each(|event_rec| {
            println!("{}", event_rec["Event"]);
        });

        // load rule files
        let mut rulefile_loader = ParseYaml::new();
        let resutl_readdir = rulefile_loader.read_dir(DIRPATH_RULES);
        if resutl_readdir.is_err() {
            eprintln!("{}", resutl_readdir.unwrap_err());
            return;
        }

        // parse rule files
        let rules: Vec<RuleNode> = rulefile_loader
            .files
            .into_iter()
            .map(|rule_file| rule::parse_rule(rule_file))
            .collect();

        // selection rule files and collect log
        let mut message = Message::new();
        rules.iter().for_each(|rule| {
            &event_records
                .iter()
                .filter(|event_record| rule.detection.select(event_record))
                .for_each(|event_record| message.insert(Utc.ymd(1996, 2, 27).and_hms(1, 5, 1), event_record.to_string()));
        });

        // output message
        message.debug();
    }
}
