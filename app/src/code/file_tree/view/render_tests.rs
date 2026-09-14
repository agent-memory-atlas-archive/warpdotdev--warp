use std::path::PathBuf;
use std::sync::Arc;

use repo_metadata::entry::FileMetadata;
use repo_metadata::file_tree_store::FileTreeDirectoryEntryState;
use warpui::elements::{DraggableState, MouseStateHandle};
use warpui::text_layout::{ClipDirection, ClipStyle};

use super::super::FileTreeItem;
use crate::appearance::Appearance;

#[test]
fn file_and_folder_names_use_end_ellipsis_and_full_tooltips() {
    let file_name = "a-very-long-file-name-that-needs-truncation.rs";
    let folder_name = "a-very-long-folder-name-that-needs-truncation";
    let file = FileTreeItem::File {
        metadata: FileMetadata::new(PathBuf::from("/project").join(file_name), false).into(),
        depth: 1,
        mouse_state_handle: MouseStateHandle::default(),
        draggable_state: DraggableState::default(),
    };
    let folder = FileTreeItem::DirectoryHeader {
        directory: FileTreeDirectoryEntryState {
            path: Arc::new(
                warp_util::standardized_path::StandardizedPath::try_from_local(
                    &PathBuf::from("/project").join(folder_name),
                )
                .unwrap(),
            ),
            ignored: false,
            loaded: true,
        },
        depth: 1,
        mouse_state_handle: MouseStateHandle::default(),
        draggable_state: DraggableState::default(),
    };
    let appearance = Appearance::mock();

    for (item, expected_name) in [(file, file_name), (folder, folder_name)] {
        let state = item.to_render_state(None, &appearance);

        assert_eq!(state.tooltip_label(), expected_name);
        assert_eq!(state.display_name, expected_name);
        assert_eq!(state.name_clip_config().direction, ClipDirection::End);
        assert_eq!(state.name_clip_config().style, ClipStyle::Ellipsis);
    }
}
