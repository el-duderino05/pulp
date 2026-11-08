// SPDX-License-Identifier: MPL-2.0
//! The project manifest: folders, item order, statuses, labels and settings.
use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::id::Id;
use crate::schema::{SchemaKind, validate};
use crate::storage::RelPath;
use crate::version::{FORMAT_VERSION, READER_VERSION, VersionError, check_readable};

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("the manifest is not valid JSON: {0}")]
    Json(String),
    #[error("the manifest does not match its schema: {}", .0.join("; "))]
    Schema(Vec<String>),
    #[error(transparent)]
    Version(#[from] VersionError),
    #[error("no folder with id {0}")]
    FolderNotFound(Id),
    #[error("id {0} is already in the manifest")]
    DuplicateId(Id),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: u32,
    pub min_reader: u32,
    pub id: Id,
    pub name: String,
    pub tree: Vec<Node>,
    pub statuses: Vec<String>,
    pub labels: Vec<Label>,
    pub settings: Map<String, Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "node", rename_all = "lowercase")]
pub enum Node {
    Folder(FolderNode),
    Item {
        id: Id,
        #[serde(flatten)]
        extra: Map<String, Value>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FolderNode {
    pub id: Id,
    pub title: String,
    pub path: String,
    pub synopsis: String,
    pub children: Vec<Node>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    pub color: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Node {
    pub fn id(&self) -> &Id {
        match self {
            Node::Folder(f) => &f.id,
            Node::Item { id, .. } => id,
        }
    }
}

fn collect_items(nodes: &[Node], out: &mut Vec<Id>) {
    for n in nodes {
        match n {
            Node::Item { id, .. } => out.push(id.clone()),
            Node::Folder(f) => collect_items(&f.children, out),
        }
    }
}

fn collect_ids(node: &Node, out: &mut Vec<Id>) {
    out.push(node.id().clone());
    if let Node::Folder(f) = node {
        for c in &f.children {
            collect_ids(c, out);
        }
    }
}

fn contains_id(nodes: &[Node], id: &Id) -> bool {
    nodes
        .iter()
        .any(|n| n.id() == id || matches!(n, Node::Folder(f) if contains_id(&f.children, id)))
}

fn find_folder<'a>(nodes: &'a [Node], id: &Id) -> Option<&'a FolderNode> {
    for n in nodes {
        if let Node::Folder(f) = n {
            if f.id == *id {
                return Some(f);
            }
            if let Some(found) = find_folder(&f.children, id) {
                return Some(found);
            }
        }
    }
    None
}

fn find_folder_mut<'a>(nodes: &'a mut [Node], id: &Id) -> Option<&'a mut FolderNode> {
    for n in nodes {
        if let Node::Folder(f) = n {
            if f.id == *id {
                return Some(f);
            }
            if let Some(found) = find_folder_mut(&mut f.children, id) {
                return Some(found);
            }
        }
    }
    None
}

fn find_folder_by_path<'a>(nodes: &'a [Node], path: &str) -> Option<&'a FolderNode> {
    for n in nodes {
        if let Node::Folder(f) = n {
            if f.path == path {
                return Some(f);
            }
            if let Some(found) = find_folder_by_path(&f.children, path) {
                return Some(found);
            }
        }
    }
    None
}

fn find_parent(nodes: &[Node], parent: Option<&Id>, id: &Id) -> Option<(Option<Id>, usize)> {
    for (i, n) in nodes.iter().enumerate() {
        if n.id() == id {
            return Some((parent.cloned(), i));
        }
        if let Node::Folder(f) = n
            && let Some(found) = find_parent(&f.children, Some(&f.id), id)
        {
            return Some(found);
        }
    }
    None
}

fn remove_from(nodes: &mut Vec<Node>, id: &Id) -> Option<Node> {
    if let Some(i) = nodes.iter().position(|n| n.id() == id) {
        return Some(nodes.remove(i));
    }
    for n in nodes {
        if let Node::Folder(f) = n
            && let Some(removed) = remove_from(&mut f.children, id)
        {
            return Some(removed);
        }
    }
    None
}

/// Checks every folder path and that no id appears twice.
fn check_tree(nodes: &[Node], seen: &mut HashSet<Id>) -> Result<(), ManifestError> {
    for n in nodes {
        if !seen.insert(n.id().clone()) {
            return Err(ManifestError::DuplicateId(n.id().clone()));
        }
        if let Node::Folder(f) = n {
            if RelPath::new(&f.path).is_err() {
                return Err(ManifestError::Schema(vec![format!(
                    "folder path {:?} is not a valid relative path",
                    f.path
                )]));
            }
            check_tree(&f.children, seen)?;
        }
    }
    Ok(())
}

/// Stores every folder path in `RelPath` form, so `\` becomes `/` (R68).
/// Run after `check_tree`, which has checked that each path is valid.
fn slash_paths(nodes: &mut [Node]) {
    for n in nodes {
        if let Node::Folder(f) = n {
            if let Ok(p) = RelPath::new(&f.path) {
                f.path = p.as_str().to_string();
            }
            slash_paths(&mut f.children);
        }
    }
}

impl Manifest {
    pub fn new(name: &str) -> Manifest {
        Manifest {
            format: FORMAT_VERSION,
            min_reader: READER_VERSION,
            id: Id::new(),
            name: name.to_string(),
            tree: vec![Node::Folder(FolderNode {
                id: Id::new(),
                title: "Manuscript".to_string(),
                path: "manuscript".to_string(),
                synopsis: String::new(),
                children: Vec::new(),
                extra: Map::new(),
            })],
            statuses: ["Idea", "Draft", "Revised", "Final"]
                .map(String::from)
                .to_vec(),
            labels: Vec::new(),
            settings: Map::new(),
            extra: Map::new(),
        }
    }

    pub fn load(text: &str) -> Result<Manifest, ManifestError> {
        let value: Value =
            serde_json::from_str(text).map_err(|e| ManifestError::Json(e.to_string()))?;
        // A too-new file is reported as such even when its body no longer fits
        // this build's schema.
        if let Some(n) = value.get("minReader").and_then(Value::as_u64) {
            check_readable(u32::try_from(n).unwrap_or(u32::MAX))?;
        }
        validate(SchemaKind::Manifest, &value).map_err(ManifestError::Schema)?;
        let mut manifest: Manifest = serde_json::from_value(value)
            .map_err(|e| ManifestError::Schema(vec![e.to_string()]))?;
        check_tree(&manifest.tree, &mut HashSet::new())?;
        slash_paths(&mut manifest.tree);
        Ok(manifest)
    }

    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).expect("a manifest always serialises");
        s.push('\n');
        s
    }

    /// Every item id, in tree order.
    pub fn item_ids(&self) -> Vec<Id> {
        let mut out = Vec::new();
        collect_items(&self.tree, &mut out);
        out
    }

    pub fn folder(&self, id: &Id) -> Option<&FolderNode> {
        find_folder(&self.tree, id)
    }

    pub fn folder_mut(&mut self, id: &Id) -> Option<&mut FolderNode> {
        find_folder_mut(&mut self.tree, id)
    }

    /// Finds a folder by its exact `path` string.
    pub fn folder_by_path(&self, path: &str) -> Option<&FolderNode> {
        find_folder_by_path(&self.tree, path)
    }

    /// The folder holding `id` (`None` for the top level) and its index there.
    pub fn parent_of(&self, id: &Id) -> Option<(Option<Id>, usize)> {
        find_parent(&self.tree, None, id)
    }

    /// Inserts an item into `folder` (`None` for the top level). An index past
    /// the end, or `None`, appends.
    pub fn insert_item(
        &mut self,
        folder: Option<&Id>,
        item: Id,
        index: Option<usize>,
    ) -> Result<(), ManifestError> {
        let node = Node::Item {
            id: item,
            extra: Map::new(),
        };
        self.insert_node(folder, node, index)
    }

    /// Inserts a node, with everything under it, into `folder` (`None` for the
    /// top level), clamping the index like `insert_item`. If any id in the
    /// subtree is already in the tree, or appears twice in it, nothing changes.
    pub fn insert_node(
        &mut self,
        folder: Option<&Id>,
        node: Node,
        index: Option<usize>,
    ) -> Result<(), ManifestError> {
        let mut ids = Vec::new();
        collect_ids(&node, &mut ids);
        let mut seen = HashSet::new();
        let dup = ids
            .into_iter()
            .find(|id| contains_id(&self.tree, id) || !seen.insert(id.clone()));
        let siblings = match folder {
            None => &mut self.tree,
            Some(fid) => {
                &mut find_folder_mut(&mut self.tree, fid)
                    .ok_or_else(|| ManifestError::FolderNotFound(fid.clone()))?
                    .children
            }
        };
        if let Some(id) = dup {
            return Err(ManifestError::DuplicateId(id));
        }
        let at = index.map_or(siblings.len(), |i| i.min(siblings.len()));
        siblings.insert(at, node);
        Ok(())
    }

    /// Removes a folder (with its children) or an item, wherever it is.
    pub fn remove_node(&mut self, id: &Id) -> Option<Node> {
        remove_from(&mut self.tree, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn top_folder(m: &Manifest) -> Id {
        match &m.tree[0] {
            Node::Folder(f) => f.id.clone(),
            _ => unreachable!(),
        }
    }

    fn as_value(m: &Manifest) -> Value {
        serde_json::from_str(&m.to_json()).unwrap()
    }

    fn keys(v: &Value) -> Vec<&str> {
        v.as_object().unwrap().keys().map(String::as_str).collect()
    }

    /// A manifest with a nested folder `manuscript/part-1` holding one item.
    fn nested() -> (Manifest, Id, Id, Id) {
        let mut m = Manifest::new("X");
        let top = top_folder(&m);
        let (part, item) = (Id::new(), Id::new());
        m.folder_mut(&top)
            .unwrap()
            .children
            .push(Node::Folder(FolderNode {
                id: part.clone(),
                title: "Part 1".into(),
                path: "manuscript/part-1".into(),
                synopsis: String::new(),
                children: vec![],
                extra: Map::new(),
            }));
        m.insert_item(Some(&part), item.clone(), None).unwrap();
        (m, top, part, item)
    }

    #[test]
    fn unknown_keys_on_items_and_labels_survive_in_order() {
        let mut m = Manifest::new("X");
        let top = top_folder(&m);
        m.insert_item(Some(&top), Id::new(), None).unwrap();
        let mut v = as_value(&m);
        v["tree"][0]["children"][0]["zeta"] = json!(1);
        v["tree"][0]["children"][0]["alpha"] = json!(2);
        v["labels"] = json!([{ "name": "Red", "color": "#f00", "zz": 1, "aa": 2 }]);
        let loaded = Manifest::load(&serde_json::to_string(&v).unwrap()).unwrap();
        let out = as_value(&loaded);
        assert_eq!(
            keys(&out["tree"][0]["children"][0]),
            ["node", "id", "zeta", "alpha"]
        );
        assert_eq!(keys(&out["labels"][0]), ["name", "color", "zz", "aa"]);
        assert_eq!(
            keys(&out["tree"][0]),
            ["node", "id", "title", "path", "synopsis", "children"]
        );
    }

    #[test]
    fn too_new_manifest_reports_version_even_when_the_body_is_broken() {
        let mut v = as_value(&Manifest::new("X"));
        v["minReader"] = json!(99);
        v["tree"] = json!("not a tree");
        match Manifest::load(&serde_json::to_string(&v).unwrap()) {
            Err(ManifestError::Version(_)) => {}
            other => panic!("expected Version, got {other:?}"),
        }
    }

    #[test]
    fn not_json_is_a_json_error() {
        assert!(matches!(
            Manifest::load("{nope"),
            Err(ManifestError::Json(_))
        ));
    }

    #[test]
    fn bad_folder_paths_are_refused_and_nested_ones_accepted() {
        for bad in ["../x", "C:/x", ""] {
            let mut v = as_value(&Manifest::new("X"));
            v["tree"][0]["path"] = json!(bad);
            let r = Manifest::load(&serde_json::to_string(&v).unwrap());
            assert!(matches!(r, Err(ManifestError::Schema(_))), "{bad:?}: {r:?}");
        }
        let (m, ..) = nested();
        let loaded = Manifest::load(&m.to_json()).unwrap();
        assert!(loaded.folder_by_path("manuscript/part-1").is_some());
        assert!(loaded.folder_by_path("part-1").is_none());
    }

    #[test]
    fn a_bad_nested_folder_path_names_the_path() {
        let (m, ..) = nested();
        let mut v = as_value(&m);
        v["tree"][0]["children"][0]["path"] = json!("a/../b");
        match Manifest::load(&serde_json::to_string(&v).unwrap()) {
            Err(ManifestError::Schema(msgs)) => assert!(msgs[0].contains("a/../b"), "{msgs:?}"),
            other => panic!("expected Schema, got {other:?}"),
        }
    }

    #[test]
    fn duplicate_ids_are_refused_on_load() {
        let (m, _, part, item) = nested();
        for dup in [item, part] {
            let mut v = as_value(&m);
            v["tree"][0]["children"]
                .as_array_mut()
                .unwrap()
                .push(json!({ "node": "item", "id": dup.as_str() }));
            let r = Manifest::load(&serde_json::to_string(&v).unwrap());
            assert!(
                matches!(r, Err(ManifestError::DuplicateId(ref d)) if *d == dup),
                "{r:?}"
            );
        }
    }

    #[test]
    fn insert_with_an_existing_id_is_refused_and_changes_nothing() {
        let (mut m, top, _, item) = nested();
        let before = m.clone();
        for target in [Some(&top), None] {
            let r = m.insert_item(target, item.clone(), Some(0));
            assert!(matches!(r, Err(ManifestError::DuplicateId(ref d)) if *d == item));
        }
        let r = m.insert_item(None, top.clone(), None);
        assert!(matches!(r, Err(ManifestError::DuplicateId(_))));
        assert_eq!(m, before);
    }

    #[test]
    fn insert_into_an_unknown_folder_fails() {
        let mut m = Manifest::new("X");
        let before = m.clone();
        let ghost = Id::new();
        let r = m.insert_item(Some(&ghost), Id::new(), None);
        assert!(matches!(r, Err(ManifestError::FolderNotFound(ref f)) if *f == ghost));
        assert_eq!(m, before);
    }

    #[test]
    fn insert_clamps_the_index_and_none_folder_is_top_level() {
        let mut m = Manifest::new("X");
        let top = top_folder(&m);
        let (a, b, c) = (Id::new(), Id::new(), Id::new());
        m.insert_item(Some(&top), a.clone(), None).unwrap();
        m.insert_item(Some(&top), b.clone(), Some(99)).unwrap();
        assert_eq!(m.item_ids(), vec![a.clone(), b.clone()]);
        assert_eq!(m.parent_of(&b), Some((Some(top.clone()), 1)));
        m.insert_item(None, c.clone(), Some(99)).unwrap();
        assert_eq!(m.parent_of(&c), Some((None, 1)));
        assert!(matches!(&m.tree[1], Node::Item { extra, .. } if extra.is_empty()));
    }

    #[test]
    fn parent_of_and_remove_node_reach_into_nested_folders() {
        let (mut m, _, part, item) = nested();
        assert_eq!(m.parent_of(&item), Some((Some(part.clone()), 0)));
        assert!(m.folder(&part).is_some());
        assert!(m.parent_of(&Id::new()).is_none());
        assert!(matches!(m.remove_node(&item), Some(Node::Item { .. })));
        assert!(m.item_ids().is_empty());
        assert!(m.remove_node(&item).is_none());
    }

    fn folder_with(items: &[Id]) -> Node {
        Node::Folder(FolderNode {
            id: Id::new(),
            title: "Sub".into(),
            path: "manuscript/sub".into(),
            synopsis: String::new(),
            children: items
                .iter()
                .map(|i| Node::Item {
                    id: i.clone(),
                    extra: Map::new(),
                })
                .collect(),
            extra: Map::new(),
        })
    }

    #[test]
    fn insert_node_places_a_subtree_and_clamps_the_index() {
        let (mut m, top, part, _) = nested();
        let node = folder_with(&[Id::new()]);
        let nid = node.id().clone();
        m.insert_node(Some(&top), node, Some(99)).unwrap();
        assert_eq!(m.parent_of(&nid), Some((Some(top.clone()), 1)));
        let node = folder_with(&[]);
        let nid = node.id().clone();
        m.insert_node(Some(&part), node, Some(0)).unwrap();
        assert_eq!(m.parent_of(&nid), Some((Some(part), 0)));
        let node = folder_with(&[]);
        let nid = node.id().clone();
        m.insert_node(None, node, None).unwrap();
        let n = m.tree.len() - 1;
        assert_eq!(m.parent_of(&nid), Some((None, n)));
    }

    #[test]
    fn insert_node_refuses_any_duplicate_id_and_changes_nothing() {
        let (mut m, top, _, item) = nested();
        let before = m.clone();
        let r = m.insert_node(Some(&top), folder_with(&[Id::new(), item.clone()]), None);
        assert!(matches!(r, Err(ManifestError::DuplicateId(ref i)) if *i == item));
        assert_eq!(m, before);
        let twice = Id::new();
        let r = m.insert_node(None, folder_with(&[twice.clone(), twice.clone()]), None);
        assert!(matches!(r, Err(ManifestError::DuplicateId(ref i)) if *i == twice));
        assert_eq!(m, before);
        let r = m.insert_node(Some(&Id::new()), folder_with(&[]), None);
        assert!(matches!(r, Err(ManifestError::FolderNotFound(_))));
        assert_eq!(m, before);
    }

    #[test]
    fn removing_a_folder_returns_it_with_its_children() {
        let (mut m, _, part, item) = nested();
        match m.remove_node(&part) {
            Some(Node::Folder(f)) => {
                assert_eq!(f.id, part);
                assert_eq!(f.children.len(), 1);
                assert_eq!(f.children[0].id(), &item);
            }
            other => panic!("expected the folder, got {other:?}"),
        }
        assert!(m.folder(&part).is_none());
        assert!(m.item_ids().is_empty());
    }
}
