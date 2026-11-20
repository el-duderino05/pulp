// SPDX-License-Identifier: MPL-2.0
// One typed function per Tauri command. Mirrors src-tauri DTOs exactly.
import { invoke } from '@tauri-apps/api/core';

export type AppError = { code: string; detail: string };

export type TreeNode = {
  id: string;
  title: string;
  kind: 'folder' | 'document' | 'missing';
  children: TreeNode[];
};

export type ReportView = {
  found: string[];
  missing: string[];
  duplicates: string[];
  copies: string[];
  conflicts: string[];
  recreated: string[];
};

export type ProjectRow = {
  id: string;
  name: string;
  path: string;
  tooNew: boolean;
  lastOpened: string | null;
  external: boolean;
};

export type OpenedProject = {
  id: string;
  name: string;
  path: string;
  sync: string | null;
  tree: TreeNode[];
  report: ReportView;
  firstDocument: string | null;
};

export type TrashRow = {
  entryId: string;
  title: string;
  isFolder: boolean;
  itemCount: number;
  from: string | null;
  deletedAt: string;
};

export type LibraryTrashEntry = { entryId: string; projectId: string; name: string; deletedAt: string };

export type LibraryInfo = { root: string; author: string; sync: string | null; recreated: string[] };

export type DocumentView = { title: string; body: string };

export type Startup =
  | { state: 'firstLaunch'; defaultPath: string }
  | { state: 'ready'; library: LibraryInfo }
  | { state: 'libraryMissing'; path: string }
  | { state: 'failed'; error: AppError };

// Library
export const startup = () => invoke<Startup>('startup');
export const createLibrary = (path: string, author: string) => invoke<LibraryInfo>('create_library', { path, author });
export const openLibrary = (path: string) => invoke<LibraryInfo>('open_library', { path });
export const listProjects = () => invoke<ProjectRow[]>('list_projects');
export const createProject = (name: string) => invoke<OpenedProject>('create_project', { name });
export const addExternalProject = (path: string) => invoke<ProjectRow>('add_external_project', { path });
export const forgetExternalProject = (path: string) => invoke<void>('forget_external_project', { path });
export const trashProject = (path: string) => invoke<void>('trash_project', { path });
export const listLibraryTrash = () => invoke<LibraryTrashEntry[]>('list_library_trash');
export const restoreProject = (entry: string) => invoke<ProjectRow>('restore_project', { entry });
export const deleteLibraryTrashEntry = (entry: string) => invoke<void>('delete_library_trash_entry', { entry });
export const emptyLibraryTrash = () => invoke<number>('empty_library_trash');

// Project
export const openProject = (path: string) => invoke<OpenedProject>('open_project', { path });
export const closeProject = (project: string) => invoke<void>('close_project', { project });
export const projectTree = (project: string) => invoke<TreeNode[]>('project_tree', { project });
export const createDocument = (project: string, folder: string, title: string) =>
  invoke<string>('create_document', { project, folder, title });
export const createFolder = (project: string, parent: string | null, title: string) =>
  invoke<string>('create_folder', { project, parent, title });
export const renameNode = (project: string, id: string, title: string) =>
  invoke<void>('rename_node', { project, id, title });
export const reorderNode = (project: string, id: string, index: number) =>
  invoke<void>('reorder_node', { project, id, index });
export const moveDocument = (project: string, id: string, folder: string | null, index: number) =>
  invoke<void>('move_document', { project, id, folder, index });
export const trashNode = (project: string, id: string) => invoke<string>('trash_node', { project, id });
export const listProjectTrash = (project: string) => invoke<TrashRow[]>('list_project_trash', { project });
export const restoreTrashEntry = (project: string, entry: string) =>
  invoke<void>('restore_trash_entry', { project, entry });
export const deleteTrashEntry = (project: string, entry: string) =>
  invoke<void>('delete_trash_entry', { project, entry });
export const emptyProjectTrash = (project: string) => invoke<number>('empty_project_trash', { project });
export const readDocument = (project: string, id: string) => invoke<DocumentView>('read_document', { project, id });

