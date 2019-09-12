use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::io::prelude::*;
use std::io::{self, BufReader};

use lazy_static::lazy_static;
use regex::{Captures, Regex};
use serde::Serialize;

use crate::report::*;



lazy_static! {
    static ref ERROR: Regex = Regex::new(
        r#"^(?:! ((?:La|pdf)TeX|Package|Class)(?: (\w+))? [eE]rror(?: \(([\\]?\w+)\))?: (.*)|! (.*))"#
    ).unwrap();

    static ref WARNING: Regex = Regex::new(
        r#"^((?:La|pdf)TeX|Package|Class)(?: (\w+))? [wW]arning(?: \(([\\]?\w+)\))?: (.*)"#
    ).unwrap();

    static ref INFO: Regex = Regex::new(
         r#"^((?:La|pdf)TeX|Package|Class)(?: (\w+))? [iI]nfo(?: \(([\\]?\w+)\))?: (.*)"#
    ).unwrap();

    static ref BADBOX: Regex = Regex::new(
        r#"^(Over|Under)full \\([hv])box \((?:badness (\d+)|(\d+(?:\.\d+)?pt) too \w+)\) (?:(?:(?:in paragraph|in alignment|detected) (?:at lines (\d+)--(\d+)|at line (\d+)))|(?:has occurred while [\\]output is active [\[](\d+)?[\]]))"#
    ).unwrap();

    static ref MISSING_REFERENCE: Regex = Regex::new(
        r#"^(Citation|Reference) `([^']+)' on page \d+ undefined on input line \d+."#
    ).unwrap();
}

struct LogParser<'a, B: 'a + BufRead> {
    report: &'a mut BuildReport,
    reader: B,
    lineno: usize,
    collect_remaining: usize,
    context_lines: usize,
}

impl<'a, B: 'a + BufRead> LogParser<'a, B> {
    fn next_line(&mut self) -> Option<String> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(read) if read > 0 => {
                self.lineno += 1;
                Some(line)
            }
            Ok(_) => None,
            Err(_) => None,
        }
    }

    fn after_match(&mut self) {
        self.collect_remaining = self.context_lines;
    }

    fn parse_line(&mut self, line: &str) {
        if let Some(m) = INFO.captures(&line) {
            self.process_info(m);
            //self.after_match();
        } else if let Some(m) = BADBOX.captures(&line) {
            self.process_badbox(m);
            //self.after_match();
        } else if let Some(m) = WARNING.captures(&line) {
            self.process_warning(m);
            //self.after_match();
        } else if let Some(m) = ERROR.captures(&line) {
            self.process_error(m);
            //self.after_match();
        }
    }

    fn process_generic(&mut self, m: Captures) -> MessageInfo {
        let mut info = MessageInfo {
            full: m.get(0).unwrap().as_str().to_owned(),
            details: HashMap::new(),
            context_lines: Vec::new(),
        };

        // 0 - Whole match
        // 1 - Type ((?:La|pdf)TeX|Package|Class)
        // 2 - Package or Class name (\w*)?
        // 3 - extra?
        // 4 - message (.*)

        let type_name = m.get(1).unwrap().as_str();
        info.details
            .insert(String::from("type"), type_name.to_owned());
        if let Some(name) = m.get(2) {
            let key = match type_name {
                "Package" => String::from("package"),
                "Class" => String::from("class"),
                _ => String::from("component"),
            };
            info.details.insert(key, name.as_str().to_owned());
        }

        if let Some(extra) = m.get(3) {
            info.details
                .insert(String::from("extra"), extra.as_str().to_owned());
        }

        info.details.insert(
            String::from("message"),
            m.get(4).unwrap().as_str().to_owned(),
        );

        info
    }

    fn process_info(&mut self, m: Captures) {
        let info = self.process_generic(m);
        self.report.info += 1;
        self.report.messages.push(Message::Info(info));
    }

    fn process_badbox(&mut self, m: Captures) {
        let mut info = MessageInfo {
            full: m.get(0).unwrap().as_str().to_owned(),
            details: HashMap::new(),
            context_lines: Vec::new(),
        };

        // Regex match groups
        // 0 - Whole match
        // 1 - type (Over|Under)
        // 2 - direction ([hv])
        // 3 - underfull box badness (badness (\d+))?
        // 4 - overfull box size (\d+(\.\d+)?pt too \w+)?
        // 5 - Multi-line start line (at lines (\d+)--)?
        // 6 - Multi-line end line (--(\d+))?
        // 7 - Single line (at line (\d+))?
        // 8 - page ([(\d+)?)?

        let box_type = m.get(1).unwrap().as_str();
        let direction = m.get(2).unwrap().as_str();
        info.details
            .insert(String::from("type"), box_type.to_owned());
        info.details
            .insert(String::from("direction"), direction.to_owned());

        if box_type == "Over" {
            let over_by = m.get(4).unwrap().as_str();
            info.details.insert(String::from("by"), over_by.to_owned());
        } else if box_type == "Under" {
            let badness = m.get(3).unwrap().as_str();
            info.details.insert(String::from("by"), badness.to_owned());
        }

        if let Some(line) = m.get(7) {
            // single line
            info.details
                .insert(String::from("line"), line.as_str().to_owned());
        } else if let Some(start) = m.get(5) {
            info.details
                .insert(String::from("start_line"), start.as_str().to_owned());
            info.details.insert(
                String::from("end_line"),
                m.get(6).unwrap().as_str().to_owned(),
            );
        }
        
        if let Some(page) = m.get(8) {
            info.details
                .insert(String::from("page"), page.as_str().to_owned());
        }

        self.report.badboxes += 1;
        self.report.messages.push(Message::Badbox(info));
    }


    fn process_missing_reference(&mut self, label: &str) {
        self.report.missing_references += 1;
        self.report.messages.push(
            Message::MissingReference {label: label.to_owned()}
        )
    }

    fn process_missing_citation(&mut self, label: &str) {
        self.report.missing_citations += 1;
        self.report.messages.push(
            Message::MissingCitation {label: label.to_owned()}
        )
    }

    fn process_warning(&mut self, m: Captures) {
        let info = self.process_generic(m);
        if let Some(message) = info.details.get("message") {
            if let Some(m) = MISSING_REFERENCE.captures(message) {
                // 0 - whole match
                // 1 - type
                // 2 - label
                let type_ = m.get(1).unwrap().as_str();
                if type_ == "Citation" {
                    self.process_missing_citation(m.get(2).unwrap().as_str());
                } else if type_ == "Reference" {
                    self.process_missing_reference(m.get(2).unwrap().as_str());
                }
                return
            }
        }
        self.report.warnings += 1;
        self.report.messages.push(Message::Warning(info));
    }

    fn process_error(&mut self, m: Captures) {
        if let Some(message) = m.get(5) {
            let mut info = MessageInfo {
                full: m.get(0).unwrap().as_str().to_owned(),
                details: HashMap::new(),
                context_lines: Vec::new(),
            };

            info.details
                .insert(String::from("message"), message.as_str().to_owned());
            self.report.errors += 1;
            self.report.messages.push(Message::Error(info))
        } else {
            let info = self.process_generic(m);
            self.report.errors += 1;
            self.report.messages.push(Message::Error(info))
        }
    }
}

impl<'a, B: 'a + BufRead> LogParser<'a, B> {
    pub fn new(report: &'a mut BuildReport, reader: B, context_lines: usize) -> LogParser<'a, B> {
        LogParser {
            report,
            reader,
            lineno: 0,
            collect_remaining: 0,
            context_lines,
        }
    }

    pub fn parse(mut self) {
        while let Some(line) = self.next_line() {

            if let Some(last) = self.report.messages.last_mut() {
                if let Some(cmpt) = last.get_component_name() {
                    let pattern = format!("({}) ", cmpt);
                    if line.starts_with(&pattern) {
                        let message = line.trim_start_matches(&pattern).trim_start();
                        last.extend_message(&message);
                        self.collect_remaining = 0;
                        continue;
                    }
                }

                //if self.collect_remaining > 0 {
                //    last.add_context(line);
                //    self.collect_remaining -= 1;
                //    continue;
                //}
            }

            self.parse_line(&line);
        }
    }
}

pub fn parse_log<R: Read>(log: R) -> BuildReport {
    let reader = BufReader::new(log);
    let mut report = BuildReport::new();

    let parser: LogParser<BufReader<R>> = LogParser::new(&mut report, reader, 2);

    parser.parse();

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_parser(line: &str) -> BuildReport {
        let mut cursor = io::Cursor::new(&line);
        let mut reader = BufReader::new(cursor);
        let mut report = BuildReport::new();
        let mut parser = LogParser::new(&mut report, reader, 2);
        parser.parse_line(&line);
        report
    }

    #[test]
    fn test_underfull_vbox_while_output_active() {
        let line = "Underfull \\vbox (badness 1234) has occurred while \\output is active []";
        let report = create_parser(&line);

        assert_eq!(report.badboxes, 1);
        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 0);
        assert_eq!(report.info, 0);
    }

    #[test]
    fn test_underfull_vbox_detected_at() {
        let line = "Underfull \\vbox (badness 10000) detected at line 19";
        let report = create_parser(&line);

        assert_eq!(report.badboxes, 1);
        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 0);
        assert_eq!(report.info, 0);
    }

    #[test]
    fn test_underfull_hbox_at_lines() {
        let line = "Underfull \\hbox (badness 1234) in paragraph at lines 9--10";
        let report = create_parser(&line);

        assert_eq!(report.badboxes, 1);
        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 0);
        assert_eq!(report.info, 0);
    }

    #[test]
    fn test_overfull_vbox_while_output_active() {
        let line = "Overfull \\vbox (19.05511pt too high) has occurred while \\output is active []";
        let report = create_parser(&line);

        assert_eq!(report.badboxes, 1);
        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 0);
        assert_eq!(report.info, 0);
    }

    #[test]
    fn test_overfull_hbox_on_line() {
        let line = "Overfull \\hbox (54.95697pt too wide) in paragraph at lines 397--397";
        let report = create_parser(&line);

        assert_eq!(report.badboxes, 1);
        assert_eq!(report.errors, 0);
        assert_eq!(report.warnings, 0);
        assert_eq!(report.info, 0);
    }

    #[test]
    fn test_package_not_found_error() {
        let line = "! LaTeX Error: File `foobar.sty' not found.";
        let report = create_parser(&line);

        assert_eq!(report.errors, 1);
    }

    #[test]
    fn test_undefined_control_sequence_tex_error() {
        let line = "! Undefined control sequence.";
        let report = create_parser(&line);

        assert_eq!(report.errors, 1);
    }

    #[test]
    fn test_too_many_braces_tex_error() {
        let line = "! Too many }'s.";
        let report = create_parser(&line);

        assert_eq!(report.errors, 1);
    }

    #[test]
    fn test_missing_math_mod_text_error() {
        let line = "! Missing $ inserted";
        let report = create_parser(&line);

        assert_eq!(report.errors, 1);
    }

    #[test]
    fn test_package_error() {
        let line = "! Package babel Error: Unknown option `latin'. Either you misspelled it";
        let report = create_parser(&line);

        assert_eq!(report.errors, 1);
    }

    #[test]
    fn test_pdftex_error() {
        let line = "! pdfTeX error (\\pdfsetmatrix): Unrecognized format..";
        let report = create_parser(&line);

        assert_eq!(report.errors, 1);
    }

    #[test]
    fn test_class_error() {
        let line = "! Class article Error: Unrecognized argument for \\macro.";
        let report = create_parser(&line);

        assert_eq!(report.errors, 1);
    }

    /*
    #[test]
    fn test_latex_undefined_reference_warning() {
        let line =
            "LaTeX Warning: Reference `undefined refr' on page 1 undefined on input line 17.";
        let report = create_parser(&line);

        assert_eq!(report.warnings, 1);
    }
    */

    #[test]
    fn test_latex_font_warning() {
        let line = "LaTeX Font Warning: Font shape `OT1/cmr/bx/sc' undefined";
        let report = create_parser(&line);

        assert_eq!(report.warnings, 1);
    }

    #[test]
    fn test_package_warning() {
        let line = "Package hyperref Warning: Draft mode on.";
        let report = create_parser(&line);

        assert_eq!(report.warnings, 1);
    }

    #[test]
    fn test_class_warning() {
        let line = "Class article Warning: Unknown option `foo'.";
        let report = create_parser(&line);

        assert_eq!(report.warnings, 1);
    }

    #[test]
    fn test_missing_reference_warning() {
        let line = "LaTeX Warning: Reference `not present' on page 1 undefined on input line 7.";
        let report = create_parser(&line);

        assert_eq!(report.missing_references, 1);

        if let Message::Warning(warning_message) = report.messages.get(0).unwrap() {
            let message = warning_message.details.get("message").unwrap();

            assert_eq!(message, "Reference `not present' on page 1 undefined on input line 7.")
        }
        
    }

    #[test]
    fn test_missing_citation_warning() {
        let line = "LaTeX Warning: Citation `not present' on page 1 undefined on input line 7.";
        let report = create_parser(&line);

        assert_eq!(report.missing_citations, 1);

        if let Message::Warning(warning_message) = report.messages.get(0).unwrap() {
            let message = warning_message.details.get("message").unwrap();

            assert_eq!(message, "Citation `not present' on page 1 undefined on input line 7.")
        }
        
    }
    
    #[test]
    fn test_underfull_vbox_has_occurred_with_page() {
        let line = "Underfull \\vbox (badness 10000) has occurred while \\output is active [38]";
        
        let report = create_parser(&line);
        assert_eq!(report.badboxes, 1);
    }

}
