// SPDX-License-Identifier: MPL-2.0
use pulp_core::{id::Id, manifest::*, schema::*};

#[test]
fn new_manifest_has_defaults() {
    let m = Manifest::new("The Storm");
    assert_eq!(m.statuses, vec!["Idea", "Draft", "Revised", "Final"]);
    match &m.tree[0] {
        Node::Folder(f) => {
            assert_eq!(f.title, "Manuscript");
            assert_eq!(f.path, "manuscript");
        }
        _ => panic!("first node must be the Manuscript folder"),
    }
}

#[test]
fn json_is_pretty_with_trailing_newline_and_round_trips() {
    let m = Manifest::new("The Storm");
    let j = m.to_json();
    assert!(j.ends_with("}\n"));
    assert!(j.contains("\n  \"format\": 1"));
    assert!(j.contains("\"minReader\": 1"));
    assert_eq!(Manifest::load(&j).unwrap().to_json(), j);
}

#[test]
fn unknown_keys_survive_load_and_save() {
    let mut v: serde_json::Value = serde_json::from_str(&Manifest::new("X").to_json()).unwrap();
    v["futureTopKey"] = serde_json::json!({"a": 1});
    v["tree"][0]["futureFolderKey"] = serde_json::json!(true);
    let out = Manifest::load(&serde_json::to_string(&v).unwrap())
        .unwrap()
        .to_json();
    assert!(out.contains("futureTopKey"));
    assert!(out.contains("futureFolderKey"));
}

#[test]
fn insert_find_and_remove_items() {
    let mut m = Manifest::new("X");
    let folder = match &m.tree[0] {
        Node::Folder(f) => f.id.clone(),
        _ => unreachable!(),
    };
    let (a, b) = (Id::new(), Id::new());
    m.insert_item(Some(&folder), a.clone(), None).unwrap();
    m.insert_item(Some(&folder), b.clone(), Some(0)).unwrap();
    assert_eq!(m.item_ids(), vec![b.clone(), a.clone()]);
    assert_eq!(m.parent_of(&a), Some((Some(folder.clone()), 1)));
    assert!(m.remove_node(&b).is_some());
    assert_eq!(m.item_ids(), vec![a]);
}

#[test]
fn schemas_accept_valid_and_reject_invalid() {
    let good: serde_json::Value = serde_json::from_str(&Manifest::new("X").to_json()).unwrap();
    assert!(validate(SchemaKind::Manifest, &good).is_ok());
    let mut bad = good.clone();
    bad.as_object_mut().unwrap().remove("id");
    assert!(validate(SchemaKind::Manifest, &bad).is_err());
}

#[test]
fn too_new_manifest_is_refused() {
    let mut v: serde_json::Value = serde_json::from_str(&Manifest::new("X").to_json()).unwrap();
    v["minReader"] = serde_json::json!(2);
    assert!(Manifest::load(&serde_json::to_string(&v).unwrap()).is_err());
}
