use serde::Serialize;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Serialize, Clone)]
pub struct MessageInfo {
    pub full: String,
    pub details: HashMap<String, String>,
    pub context_lines: Vec<String>,
}

impl MessageInfo {
    fn get_component_name(&self) -> Option<&String> {
        if self.details.contains_key("component") {
            Some(&self.details.get("component").unwrap())
        } else if self.details.contains_key("package") {
            Some(&self.details.get("package").unwrap())
        } else if self.details.contains_key("class") {
            Some(&self.details.get("class").unwrap())
        } else {
            None
        }
    }

    fn add_context(&mut self, line: String) {
        self.context_lines.push(line);
    }

    fn extend_message(&mut self, message: &str) {
        if let Some(current) = self.details.get_mut("message") {
            current.push_str(message);
        } else {
            self.details
                .insert(String::from("message"), message.to_owned());
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub enum Message {
    Error(MessageInfo),
    Warning(MessageInfo),
    Badbox(MessageInfo),
    Info(MessageInfo),
    MissingCitation { label: String },
    MissingReference { label: String },
}

use Message::*;

impl Message {
    pub(crate) fn get_component_name(&self) -> Option<&String> {
        match self {
            Error(ref inner) | Warning(ref inner) | Info(ref inner) => {
                inner.get_component_name()
            },
            _ => None,
        }
    }

    pub(crate) fn extend_message(&mut self, message: &str) {
        self.as_mut().unwrap().extend_message(message);
    }

    pub(crate) fn add_context(&mut self, line: String) {
        if let Some(inner) = self.as_mut() {
            inner.add_context(line);
        }
    }

    pub fn as_ref(&self) -> Option<&MessageInfo> {
        match self {
            Error(ref inner) => Some(inner),
            Warning(ref inner) => Some(inner),
            Badbox(ref inner) => Some(inner),
            Info(ref inner) => Some(inner),
            _ => None
        }
    }

    pub fn as_mut(&mut self) -> Option<&mut MessageInfo> {
        match self {
            Error(ref mut inner) => Some(inner),
            Warning(ref mut inner) => Some(inner),
            Badbox(ref mut inner) => Some(inner),
            Info(ref mut inner) => Some(inner),
            _ => None
        }
    }

    pub fn to_str(&self) -> String {
        use Message::*;
        match self {
            Error(ref inner) => inner.full.clone(),
            Warning(ref inner) => inner.full.clone(),
            Info(ref inner) => inner.full.clone(),
            Badbox(ref inner) => inner.full.clone(),
            MissingCitation { label } => format!("Missing citation: {}", &label),
            MissingReference { label } => format!("Missing reference: {}", &label),
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct BuildReport {
    pub errors: usize,
    pub warnings: usize,
    pub badboxes: usize,
    pub info: usize,
    pub missing_references: usize,
    pub missing_citations: usize,
    pub messages: Vec<Message>,
}

impl BuildReport {
    pub(crate) fn new() -> BuildReport {
        BuildReport {
            messages: Vec::new(),
            errors: 0,
            warnings: 0,
            badboxes: 0,
            info: 0,
            missing_citations: 0,
            missing_references: 0,
        }
    }
}

impl fmt::Display for BuildReport {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Errors: {}, Warnings: {}, Badboxes: {}",
            self.errors,
            self.warnings,
            self.badboxes,
        )
    }

}