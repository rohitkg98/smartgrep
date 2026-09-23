// Call-graph fixture for the Rust parser.

use std::collections::{HashMap, hash_map::Entry};
use std::fmt::{self, Display};
use crate::store::load as load_store;
use crate::ir::types::*;

pub struct Registry {
    items: Vec<u32>,
}

pub trait Describe {
    fn label(&self) -> String;

    fn describe(&self) -> String {
        let l = self.label();
        helper(&l)
    }
}

impl Registry {
    pub fn new() -> Self {
        Registry { items: Vec::new() }
    }

    pub fn build(&mut self) -> Vec<u32> {
        let map: HashMap<u32, u32> = HashMap::new();
        self.items.push(1);
        self.items.push(2);
        let doubled = self.items.iter().map(|x| transform(*x)).collect::<Vec<_>>();
        println!("{}", ignored_in_macro());
        let parsed = "7".parse::<u32>();
        let _ = Some(parsed);
        fn nested() {
            deep_call();
        }
        crate::store::load();
        let _ = map;
        doubled
    }
}

impl Display for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "registry")
    }
}

pub fn helper(s: &str) -> String {
    Registry::new();
    s.to_uppercase()
}

fn transform(x: u32) -> u32 {
    x * 2
}
