// SPDX-License-Identifier: MIT
use std::collections::BTreeMap;

#[derive(Debug)]
pub enum ScopeChild {
    Signal,
    Scope(Scope),
}

#[derive(Debug)]
pub struct Scope {
    pub name: String,
    children: BTreeMap<String, ScopeChild>,
}

impl Scope {
    pub fn new(name: String) -> Scope {
        Scope {
            name,
            children: BTreeMap::new(),
        }
    }

    pub fn add_scope(&mut self, path: &[&str]) {
        if let Some(name) = path.first() {
            match self.children.get(*name) {
                Some(_) => self.get_scope(*name).unwrap().add_scope(&path[1..]),
                None => {
                    let mut s = Scope::new(name.to_string());
                    s.add_scope(&path[1..]);
                    self.children.insert(s.name.clone(), ScopeChild::Scope(s));
                }
            }
        }
    }

    fn get_child(&mut self, name: &str) -> Option<&mut ScopeChild> {
        self.children.get_mut(name)
    }

    pub fn get_scope(&mut self, name: &str) -> Option<&mut Scope> {
        self.get_child(name).map(|child| match child {
            ScopeChild::Scope(next) => next,
            _ => panic!("Specified path is a signal, not a scope"),
        })
    }

    pub fn get_scope_by_path(&mut self, path: &[&str]) -> Option<&mut Scope> {
        match path.first() {
            Some(name) => match self.children.get(*name) {
                Some(_) => self.get_scope(name).unwrap().get_scope_by_path(&path[1..]),
                None => None,
            },
            None => Some(self),
        }
    }

    pub fn add_signal(&mut self, signal_id: String) {
        self.children.insert(signal_id, ScopeChild::Signal);
    }

    fn _traverse<T, F: FnMut(&str, &ScopeChild, u64) -> T>(&self, depth: u64, f: &mut F) {
        for (name, child) in &self.children {
            f(name, child, depth);
            match child {
                ScopeChild::Signal => (),
                ScopeChild::Scope(scope) => scope._traverse(depth + 1, f),
            }
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
        s.get_scope_by_path(&["foo", "bar"])
            .expect("Path doesn't exist");
    }
}
