//! Permissions, modelled after `pocketmine\permission\*`.
//!
//! The model is simplified but behaviour compatible for plugin developers:
//! a permission has a default for "everyone" and for operators, and a
//! permissible object can hold attachments that override individual nodes.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionDefault {
    True,
    False,
    Op,
    NotOp,
}

impl PermissionDefault {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "true" | "allow" | "on" => PermissionDefault::True,
            "op" => PermissionDefault::Op,
            "notop" | "not-op" => PermissionDefault::NotOp,
            _ => PermissionDefault::False,
        }
    }

    pub fn resolve(&self, is_op: bool) -> bool {
        match self {
            PermissionDefault::True => true,
            PermissionDefault::False => false,
            PermissionDefault::Op => is_op,
            PermissionDefault::NotOp => !is_op,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    pub name: String,
    pub description: String,
    pub default: PermissionDefault,
    /// Child nodes with their required value (`node` -> allowed).
    pub children: HashMap<String, bool>,
}

impl Permission {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        default: PermissionDefault,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            default,
            children: HashMap::new(),
        }
    }
}

/// Individual permission node overrides for a permissible object.
#[derive(Debug, Clone, Default)]
pub struct PermissionAttachment {
    pub plugin: Option<String>,
    permissions: HashMap<String, bool>,
}

impl PermissionAttachment {
    pub fn new(plugin: Option<String>) -> Self {
        Self {
            plugin,
            permissions: HashMap::new(),
        }
    }

    pub fn set_permission(&mut self, node: &str, value: bool) {
        self.permissions.insert(node.to_ascii_lowercase(), value);
    }

    pub fn unset_permission(&mut self, node: &str) {
        self.permissions.remove(&node.to_ascii_lowercase());
    }

    pub fn get_permission(&self, node: &str) -> Option<bool> {
        self.permissions.get(&node.to_ascii_lowercase()).copied()
    }

    pub fn clear(&mut self) {
        self.permissions.clear();
    }

    pub fn len(&self) -> usize {
        self.permissions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.permissions.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &bool)> {
        self.permissions.iter()
    }
}

/// Registry of permission definitions, shared between the server and plugins.
#[derive(Default)]
pub struct PermissionRegistry {
    permissions: RwLock<HashMap<String, Arc<Permission>>>,
}

impl PermissionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, permission: Permission) {
        let mut map = self.permissions.write().unwrap();
        map.insert(permission.name.to_ascii_lowercase(), Arc::new(permission));
    }

    pub fn unregister(&self, name: &str) {
        self.permissions
            .write()
            .unwrap()
            .remove(&name.to_ascii_lowercase());
    }

    pub fn get(&self, name: &str) -> Option<Arc<Permission>> {
        self.permissions
            .read()
            .unwrap()
            .get(&name.to_ascii_lowercase())
            .cloned()
    }

    pub fn all(&self) -> Vec<Arc<Permission>> {
        self.permissions.read().unwrap().values().cloned().collect()
    }

    /// Resolves a node taking attachments, registry defaults and op status into
    /// account. `attachments` must be ordered from newest to oldest.
    pub fn resolve(&self, node: &str, is_op: bool, attachments: &[PermissionAttachment]) -> bool {
        for attachment in attachments {
            if let Some(value) = attachment.get_permission(node) {
                return value;
            }
        }
        if let Some(permission) = self.get(node) {
            !permission.children.is_empty()
                && permission.children.values().all(|v| *v) == false
                && permission
                    .children
                    .iter()
                    .all(|(child, required)| !*required || self.resolve(child, is_op, attachments))
                && permission.default.resolve(is_op)
                || permission.default.resolve(is_op)
        } else {
            // Undefined nodes behave like PMMP's default: operators get access,
            // everyone else does not.
            is_op
        }
    }
}

/// Objects that can carry permissions (players, console sender).
pub trait Permissible {
    fn is_op(&self) -> bool;
    fn attachments(&self) -> &[PermissionAttachment];
    fn attachments_mut(&mut self) -> &mut Vec<PermissionAttachment>;
}

pub fn has_permission<T: Permissible>(
    target: &T,
    registry: &PermissionRegistry,
    node: &str,
) -> bool {
    registry.resolve(node, target.is_op(), target.attachments())
}
