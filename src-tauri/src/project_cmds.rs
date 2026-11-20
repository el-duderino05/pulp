// SPDX-License-Identifier: MPL-2.0
use crate::error::AppError;
use crate::state::{AppState, run, update_device, with_project};
use crate::sync::sync_service;
use pulp_core::header::{DocKind, parse_markdown_item};
use pulp_core::id::Id;
use pulp_core::manifest::Node;
use pulp_core::project::{Project, ReconcileReport};
use pulp_core::storage::{FsStorage, RelPath, Storage};
use pulp_core::tree::{NodeKind, TreeNode};
use serde::Serialize;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};

/// What the interface shows about the files found or fixed while opening.
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct ReportView {
    pub found: Vec<String>,
    pub missing: Vec<String>,
    pub duplicates: Vec<String>,
    pub copies: Vec<String>,
    pub conflicts: Vec<String>,
    pub recreated: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedProject {
    pub id: String,
    pub name: String,
    pub path: String,
    pub sync: Option<String>,
    pub tree: Vec<TreeNode>,
    pub report: ReportView,
    pub first_document: Option<Id>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashRow {
    pub entry_id: Id,
    pub title: String,
    pub is_folder: bool,
    pub item_count: usize,
    pub from: Option<String>,
    pub deleted_at: String,
}

#[derive(Debug, Serialize)]
pub struct DocumentView {
    pub title: String,
    pub body: String,
}

fn title_of<S: Storage>(p: &Project<S>, id: &Id) -> String {
    p.read_item(id)
        .map_or_else(|_| id.to_string(), |i| i.header.title)
}

fn view_of(p: &Project<FsStorage>, r: &ReconcileReport) -> ReportView {
    let path = |x: &RelPath| x.as_str().to_string();
    ReportView {
        found: r.found.iter().map(|id| title_of(p, id)).collect(),
        missing: r.missing.iter().map(Id::to_string).collect(),
        duplicates: r.duplicates_trashed.iter().map(path).collect(),
        copies: r.copies_reassigned.iter().map(|(x, _)| path(x)).collect(),
        conflicts: r
            .conflicts
            .iter()
            .map(|(_, ps)| {
                ps.iter()
                    .map(RelPath::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .collect(),
        recreated: r.recreated_dirs.clone(),
    }
}

pub fn first_document(tree: &[TreeNode]) -> Option<Id> {
    tree.iter().find_map(|n| match n.kind {
        NodeKind::Document => Some(n.id.clone()),
        NodeKind::Folder => first_document(&n.children),
        NodeKind::Missing => None,
    })
}

/// Opens (or reopens) the project in `path`, keeps it for the session and
/// records when it was opened. A project that needs a newer Pulp is refused.
pub fn open_project_with(state: &AppState, path: &Path) -> Result<OpenedProject, AppError> {
    let (project, report) = Project::open(FsStorage::new(path))?;
    let id = project.manifest().id.to_string();
    let tree = project.tree();
    let opened = OpenedProject {
        id: id.clone(),
        name: project.manifest().name.clone(),
        path: path.to_string_lossy().into_owned(),
        sync: sync_service(path).map(str::to_string),
        first_document: first_document(&tree),
        report: view_of(&project, &report),
        tree,
    };
    state
        .projects
        .lock()
        .expect("projects lock")
        .insert(id.clone(), Arc::new(Mutex::new(project)));
    // The project is open either way; a failed save only loses "last opened".
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let _ = update_device(state, |d| {
        d.last_opened.insert(id, now);
    });
    Ok(opened)
}

pub fn close_with(state: &AppState, project: &str) {
    state
        .projects
        .lock()
        .expect("projects lock")
        .remove(project);
}

pub fn create_document_with(
    state: &AppState,
    project: &str,
    folder: &Id,
    title: &str,
) -> Result<Id, AppError> {
    with_project(state, project, |p| {
        p.create_document(folder, title, DocKind::Prose)
    })
}

pub fn create_folder_with(
    state: &AppState,
    project: &str,
    parent: Option<&Id>,
    title: &str,
) -> Result<Id, AppError> {
    with_project(state, project, |p| p.create_folder(parent, title))
}

pub fn rename_node_with(
    state: &AppState,
    project: &str,
    id: &Id,
    title: &str,
) -> Result<(), AppError> {
    with_project(state, project, |p| {
        if p.manifest().folder(id).is_some() {
            p.rename_folder(id, title)
        } else {
            p.rename_item(id, title).map(|_| ())
        }
    })
}

pub fn trash_node_with(state: &AppState, project: &str, id: &Id) -> Result<Id, AppError> {
    with_project(state, project, |p| {
        if p.manifest().folder(id).is_some() {
            p.trash_folder(id)
        } else {
            p.trash_item(id)
        }
    })
}

fn count_items(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|n| match n {
            Node::Item { .. } => 1,
            Node::Folder(f) => count_items(&f.children),
        })
        .sum()
}

pub fn list_trash_with(state: &AppState, project: &str) -> Result<Vec<TrashRow>, AppError> {
    with_project(state, project, |p| {
        let mut rows = Vec::new();
        for e in p.list_trash()? {
            let (title, item_count) = match &e.folder_node {
                Some(f) => (f.title.clone(), count_items(&f.children)),
                None => {
                    let file = e.files.first().cloned().unwrap_or_default();
                    let path = RelPath::root()
                        .join(".trash")
                        .join(e.entry_id.as_str())
                        .join(&file);
                    let title = p
                        .storage()
                        .read(&path)
                        .ok()
                        .and_then(|b| String::from_utf8(b).ok())
                        .and_then(|t| parse_markdown_item(&t).ok())
                        .map_or(file, |i| i.header.title);
                    (title, 1)
                }
            };
            let from = e
                .original_folder
                .as_ref()
                .and_then(|f| p.manifest().folder(f))
                .map(|f| f.title.clone());
            rows.push(TrashRow {
                entry_id: e.entry_id,
                title,
                is_folder: e.folder_node.is_some(),
                item_count,
                from,
                deleted_at: e.deleted_at,
            });
        }
        Ok(rows)
    })
}

#[tauri::command]
pub async fn open_project(app: AppHandle, path: String) -> Result<OpenedProject, AppError> {
    run(app, move |s| open_project_with(s, Path::new(&path))).await
}

#[tauri::command]
pub fn close_project(state: State<'_, AppState>, project: String) {
    close_with(&state, &project);
}

#[tauri::command]
pub async fn project_tree(app: AppHandle, project: String) -> Result<Vec<TreeNode>, AppError> {
    run(app, move |s| with_project(s, &project, |p| Ok(p.tree()))).await
}

#[tauri::command]
pub async fn create_document(
    app: AppHandle,
    project: String,
    folder: Id,
    title: String,
) -> Result<Id, AppError> {
    run(app, move |s| {
        create_document_with(s, &project, &folder, &title)
    })
    .await
}

#[tauri::command]
pub async fn create_folder(
    app: AppHandle,
    project: String,
    parent: Option<Id>,
    title: String,
) -> Result<Id, AppError> {
    run(app, move |s| {
        create_folder_with(s, &project, parent.as_ref(), &title)
    })
    .await
}

#[tauri::command]
pub async fn rename_node(
    app: AppHandle,
    project: String,
    id: Id,
    title: String,
) -> Result<(), AppError> {
    run(app, move |s| rename_node_with(s, &project, &id, &title)).await
}

#[tauri::command]
pub async fn reorder_node(
    app: AppHandle,
    project: String,
    id: Id,
    index: usize,
) -> Result<(), AppError> {
    run(app, move |s| {
        with_project(s, &project, |p| p.reorder(&id, index))
    })
    .await
}

/// Documents only; folders reorder but never move (D2).
#[tauri::command]
pub async fn move_document(
    app: AppHandle,
    project: String,
    id: Id,
    folder: Option<Id>,
    index: usize,
) -> Result<(), AppError> {
    run(app, move |s| {
        with_project(s, &project, |p| p.move_item(&id, folder.as_ref(), index))
    })
    .await
}

#[tauri::command]
pub async fn trash_node(app: AppHandle, project: String, id: Id) -> Result<Id, AppError> {
    run(app, move |s| trash_node_with(s, &project, &id)).await
}

#[tauri::command]
pub async fn list_project_trash(
    app: AppHandle,
    project: String,
) -> Result<Vec<TrashRow>, AppError> {
    run(app, move |s| list_trash_with(s, &project)).await
}

#[tauri::command]
pub async fn restore_trash_entry(
    app: AppHandle,
    project: String,
    entry: Id,
) -> Result<(), AppError> {
    run(app, move |s| {
        with_project(s, &project, |p| p.restore_from_trash(&entry))
    })
    .await
}

#[tauri::command]
pub async fn delete_trash_entry(
    app: AppHandle,
    project: String,
    entry: Id,
) -> Result<(), AppError> {
    run(app, move |s| {
        with_project(s, &project, |p| p.delete_from_trash(&entry))
    })
    .await
}

#[tauri::command]
pub async fn empty_project_trash(app: AppHandle, project: String) -> Result<usize, AppError> {
    run(app, move |s| with_project(s, &project, |p| p.empty_trash())).await
}

#[tauri::command]
pub async fn read_document(
    app: AppHandle,
    project: String,
    id: Id,
) -> Result<DocumentView, AppError> {
    run(app, move |s| {
        with_project(s, &project, |p| {
            let i = p.read_item(&id)?;
            Ok(DocumentView {
                title: i.header.title,
                body: i.body,
            })
        })
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use crate::library_cmds::create_project_with;
    use crate::state::test_state;
    use pulp_core::project::MANIFEST_FILE;
    use pulp_core::tree::NodeKind;

    fn titles(nodes: &[TreeNode]) -> Vec<&str> {
        nodes.iter().map(|n| n.title.as_str()).collect()
    }

    #[test]
    fn create_project_opens_on_chapter_1() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let o = create_project_with(&state, "Novel").unwrap();
        assert_eq!(o.tree.len(), 1);
        assert_eq!(o.tree[0].title, "Manuscript");
        assert_eq!(o.tree[0].children.len(), 1);
        assert_eq!(o.tree[0].children[0].title, "Chapter 1");
        assert_eq!(o.first_document, Some(o.tree[0].children[0].id.clone()));
    }

    #[test]
    fn parallel_creates_are_serialised() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let o = create_project_with(&state, "Novel").unwrap();
        let folder = o.tree[0].id.clone();
        std::thread::scope(|s| {
            for _ in 0..2 {
                s.spawn(|| {
                    for _ in 0..20 {
                        create_document_with(&state, &o.id, &folder, "Scene").unwrap();
                    }
                });
            }
        });
        let (p, _) = Project::open(FsStorage::new(&o.path)).unwrap();
        let scenes: Vec<_> = p.tree()[0]
            .children
            .iter()
            .filter(|n| n.title == "Scene")
            .map(|n| n.id.clone())
            .collect();
        assert_eq!(scenes.len(), 40);
        assert!(scenes.iter().all(|n| {
            p.tree()[0]
                .children
                .iter()
                .any(|c| &c.id == n && c.kind == NodeKind::Document)
        }));
        let disk = Path::new(&o.path).join(&p.manifest().folder(&folder).unwrap().path);
        let files = std::fs::read_dir(disk)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .path()
                    .extension()
                    .is_some_and(|x| x == "md")
            })
            .count();
        assert_eq!(files, 41); // 40 scenes and Chapter 1: nothing overwritten
    }

    #[test]
    fn trash_row_titles_come_from_headers() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let o = create_project_with(&state, "Novel").unwrap();
        trash_node_with(&state, &o.id, &o.first_document.unwrap()).unwrap();
        let rows = list_trash_with(&state, &o.id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "Chapter 1");
        assert_eq!(rows[0].from.as_deref(), Some("Manuscript"));
        assert!(!rows[0].is_folder);
        assert_eq!(rows[0].item_count, 1);
    }

    #[test]
    fn too_new_open_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let o = create_project_with(&state, "Novel").unwrap();
        close_with(&state, &o.id);
        let manifest = Path::new(&o.path).join(MANIFEST_FILE);
        let text = std::fs::read_to_string(&manifest).unwrap();
        let text = text.replace("\"minReader\": 1", "\"minReader\": 99");
        std::fs::write(&manifest, &text).unwrap();
        let err = open_project_with(&state, Path::new(&o.path)).unwrap_err();
        assert_eq!(err.code, ErrorCode::TooNew);
        assert_eq!(std::fs::read_to_string(&manifest).unwrap(), text);
    }

    #[test]
    fn report_view_names_found_items() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let a = create_project_with(&state, "A").unwrap();
        let folder = a.tree[0].id.clone();
        let id = create_document_with(&state, &a.id, &folder, "Stray").unwrap();
        let src = with_project(&state, &a.id, |p| {
            let rel = p.path_of(&id).unwrap().clone();
            Ok((p.storage().read(&rel)?, rel))
        })
        .unwrap();

        let b = create_project_with(&state, "B").unwrap();
        let name = src.1.file_name();
        let dest = Path::new(&b.path).join("manuscript");
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(dest.join(name), &src.0).unwrap();
        close_with(&state, &b.id);

        let o = open_project_with(&state, Path::new(&b.path)).unwrap();
        assert_eq!(o.report.found, vec!["Stray".to_string()]);
        assert!(titles(&o.tree[0].children).contains(&"Stray"));
    }

    #[test]
    fn json_shapes_are_pinned() {
        use crate::library_cmds::Startup;
        fn v(x: impl Serialize) -> serde_json::Value {
            serde_json::to_value(x).unwrap()
        }
        assert_eq!(
            v(Startup::FirstLaunch {
                default_path: "/d".into()
            }),
            serde_json::json!({"state": "firstLaunch", "defaultPath": "/d"})
        );
        assert_eq!(
            v(Startup::LibraryMissing { path: "/p".into() }),
            serde_json::json!({"state": "libraryMissing", "path": "/p"})
        );
        assert_eq!(
            v(AppError {
                code: ErrorCode::TooNew,
                detail: "d".into()
            }),
            serde_json::json!({"code": "TooNew", "detail": "d"})
        );
        let row = crate::library_cmds::ProjectRow {
            id: "i".into(),
            name: "n".into(),
            path: "p".into(),
            too_new: false,
            last_opened: None,
            external: true,
        };
        assert_eq!(
            v(row),
            serde_json::json!({"id":"i","name":"n","path":"p","tooNew":false,"lastOpened":null,"external":true})
        );
        let report = ReportView::default();
        assert_eq!(
            v(&report),
            serde_json::json!({"found":[],"missing":[],"duplicates":[],"copies":[],"conflicts":[],"recreated":[]})
        );
        let opened = OpenedProject {
            id: "i".into(),
            name: "n".into(),
            path: "p".into(),
            sync: None,
            tree: Vec::new(),
            report,
            first_document: None,
        };
        assert_eq!(
            v(opened),
            serde_json::json!({"id":"i","name":"n","path":"p","sync":null,"tree":[],"report":v(ReportView::default()),"firstDocument":null})
        );
        let t = TrashRow {
            entry_id: Id::new(),
            title: "t".into(),
            is_folder: false,
            item_count: 1,
            from: None,
            deleted_at: "x".into(),
        };
        let j = v(t);
        for k in [
            "entryId",
            "title",
            "isFolder",
            "itemCount",
            "from",
            "deletedAt",
        ] {
            assert!(j.get(k).is_some(), "{k}");
        }
    }
}
