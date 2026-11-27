// SPDX-License-Identifier: MPL-2.0
//! The project tree with display titles, for the interface.
use serde::Serialize;

use crate::id::Id;
use crate::manifest::Node;
use crate::project::{Project, ProjectError};
use crate::storage::{ErrorCause, Storage};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeNode {
    pub id: Id,
    pub title: String,
    pub kind: NodeKind,
    pub children: Vec<TreeNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeKind {
    Folder,
    Document,
    Missing,
}

impl<S: Storage> Project<S> {
    /// The manifest tree with titles. Items whose file is gone are `Missing`;
    /// items with an unreadable header show their file stem.
    pub fn tree(&self) -> Vec<TreeNode> {
        self.nodes(&self.manifest().tree)
    }

    fn nodes(&self, nodes: &[Node]) -> Vec<TreeNode> {
        nodes
            .iter()
            .map(|node| match node {
                Node::Folder(f) => TreeNode {
                    id: f.id.clone(),
                    title: f.title.clone(),
                    kind: NodeKind::Folder,
                    children: self.nodes(&f.children),
                },
                Node::Item { id, .. } => self.item_node(id),
            })
            .collect()
    }

    fn item_node(&self, id: &Id) -> TreeNode {
        let (title, kind) = match (self.path_of(id), self.read_item(id)) {
            (None, _) => (String::new(), NodeKind::Missing),
            (_, Ok(item)) => (item.header.title, NodeKind::Document),
            (_, Err(ProjectError::Storage(e))) if e.cause() == ErrorCause::NotFound => {
                (String::new(), NodeKind::Missing)
            }
            (Some(path), Err(_)) => {
                let name = path.file_name();
                let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
                (stem.to_string(), NodeKind::Document)
            }
        };
        TreeNode {
            id: id.clone(),
            title,
            kind,
            children: Vec::new(),
        }
    }
}
