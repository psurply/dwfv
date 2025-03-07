// SPDX-License-Identifier: MIT
use super::signal::Signal;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
pub enum ScopeChild {
    Signal,
    Scope,
}

#[derive(Debug)]
pub struct Scope {
    pub name: String,
    signals: BTreeSet<String>,
    scopes: BTreeMap<String, Scope>,
    path: Vec<String>,
}

impl Scope {
    pub fn new(name: String) -> Scope {
        Scope {
            name,
            signals: BTreeSet::new(),
            scopes: BTreeMap::new(),
            path: Vec::new(),
        }
    }

    pub fn add_scope(&mut self, path: &[&str]) {
        if let Some(name) = path.first() {
            match self.scopes.get(*name) {
                Some(_) => self.get_scope(name).unwrap().add_scope(&path[1..]),
                None => {
                    let mut s = Scope::new(name.to_string());
                    s.path = self.path.clone();
                    s.path.push(self.name.clone());
                    s.add_scope(&path[1..]);
                    self.scopes.insert(s.name.clone(), s);
                }
            }
        }
    }

    pub fn get_scope(&mut self, name: &str) -> Option<&mut Scope> {
        self.scopes.get_mut(name)
    }

    pub fn get_scope_by_path(&mut self, path: &[&str]) -> Option<&mut Scope> {
        match path.first() {
            Some(name) => match self.scopes.get(*name) {
                Some(_) => self.get_scope(name).unwrap().get_scope_by_path(&path[1..]),
                None => None,
            },
            None => Some(self),
        }
    }

    pub fn add_signal(&mut self, signal: &mut Signal) {
        self.signals.insert(signal.id.clone());
        signal.path = self.path.clone();
        signal.path.push(self.name.clone())
    }

    fn _traverse<T, F: FnMut(&str, &ScopeChild, u64) -> T>(&self, depth: u64, f: &mut F) {
        for name in &self.signals {
            f(name, &ScopeChild::Signal, depth);
        }

        for (name, scope) in &self.scopes {
            f(name, &ScopeChild::Scope, depth);
            scope._traverse(depth + 1, f);
        }
    }

    pub fn traverse<T, F: FnMut(&str, &ScopeChild, u64) -> T>(&self, f: &mut F) {
        self._traverse(0, f)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn scope() {
        let mut s = Scope::new(String::from("top"));
        s.add_scope(&["foo", "bar"]);

        let foo = s.get_scope_by_path(&["foo"]).expect("Path doesn't exist");
        foo.add_signal(&mut Signal::new("bar", "bar", 42));

        s.get_scope_by_path(&["foo", "bar"])
            .expect("Path doesn't exist");
    }
}
