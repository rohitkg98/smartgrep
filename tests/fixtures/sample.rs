// Test fixture for parser tests.
// Contains a variety of Rust items.

use std::collections::HashMap;
use std::fmt;

const MAX_SIZE: usize = 1024;

type Callback = fn(i32) -> bool;

#[derive(Debug, Clone)]
pub struct Config {
    pub name: String,
    pub values: Vec<i32>,
    timeout: u64,
}

pub enum Status {
    Active,
    Inactive,
    Error(String),
}

pub trait Processor {
    fn process(&self, input: &str) -> String;
    fn name(&self) -> &str;
}

impl Config {
    pub fn new(name: String) -> Self {
        Config {
            name,
            values: vec![],
            timeout: 30,
        }
    }

    pub fn add_value(&mut self, v: i32) {
        self.values.push(v);
    }
}

impl Processor for Config {
    fn process(&self, input: &str) -> String {
        format!("{}: {}", self.name, input)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

pub fn standalone_function(x: i32, y: i32) -> i32 {
    x + y
}

fn private_helper() {
    // does nothing
}
