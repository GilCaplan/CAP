use crate::interpreter::value::Value;
use std::collections::HashMap;

/// A lexical scope chain.
/// Scopes are stored innermost-first (index 0 = current scope).
#[derive(Debug, Clone)]
pub struct Env {
    scopes: Vec<HashMap<String, Value>>,
}

impl Env {
    pub fn new() -> Self {
        Env { scopes: vec![HashMap::new()] }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// Look up a variable, walking from innermost to outermost scope.
    pub fn get(&self, name: &str) -> Option<Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v.clone());
            }
        }
        None
    }

    /// Set a variable in the innermost scope (always creates/overwrites locally).
    pub fn set(&mut self, name: &str, value: Value) {
        self.scopes.last_mut().unwrap().insert(name.to_string(), value);
    }

    /// Update an existing variable in whichever scope it lives in.
    /// Returns `true` if found and updated, `false` if not found (caller should
    /// fall back to `set` to create in the current scope).
    pub fn update_existing(&mut self, name: &str, value: Value) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), value);
                return true;
            }
        }
        false
    }

    /// Assign: update if exists anywhere, otherwise create in current scope.
    pub fn assign(&mut self, name: &str, value: Value) {
        if !self.update_existing(name, value.clone()) {
            self.set(name, value);
        }
    }

    /// Return all (name, value) bindings in the topmost scope.
    pub fn current_scope_bindings(&self) -> Vec<(String, Value)> {
        self.scopes.last()
            .map(|s| s.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    }

    /// Capture the current scope chain as a closure snapshot.
    pub fn snapshot(&self) -> EnvSnapshot {
        EnvSnapshot(self.scopes.clone())
    }

    /// Restore from a snapshot, adding a fresh local scope on top.
    pub fn from_snapshot(snap: &EnvSnapshot) -> Self {
        let mut scopes = snap.0.clone();
        scopes.push(HashMap::new());
        Env { scopes }
    }
}

impl Default for Env {
    fn default() -> Self { Env::new() }
}

/// An immutable snapshot of the scope chain, used to capture closures.
#[derive(Debug, Clone)]
pub struct EnvSnapshot(pub Vec<HashMap<String, Value>>);
