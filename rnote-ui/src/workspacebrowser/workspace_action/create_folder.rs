use std::path::PathBuf;

use gettextrs::gettext;
use gtk4::{
    gio, glib,
    glib::clone,
    pango,
    traits::{BoxExt, ButtonExt, EditableExt, PopoverExt, StyleContextExt, WidgetExt},
    Align, Button, Entry, Label, Popover,
};

use crate::{workspacebrowser::widget_helper, WorkspaceBrowser};

/// Creates a new `create_folder` action
pub fn create_folder(workspacebrowser: &WorkspaceBrowser) -> gio::SimpleAction {
    let new_folder_action = gio::SimpleAction::new("create-folder", None);

    new_folder_action.connect_activate(clone!(@weak workspacebrowser as workspacebrowser => move |_, _| {
        if let Some(parent_path) = workspacebrowser.selected_workspace_dir() {
            let folder_name_entry = create_folder_name_entry();
            let dialog_title_label = create_dialog_title_label();
            let (apply_button, popover) = widget_helper::entry_dialog::create_entry_dialog(&folder_name_entry, &dialog_title_label);

            // at first don't allow applying, since the user did not enter any text yet.
            apply_button.set_sensitive(false);

            workspacebrowser.dir_controls_actions_box().append(&popover);

            folder_name_entry.connect_changed(clone!(@weak apply_button, @strong parent_path => move |entry| {
                let entry_text = entry.text();
                let new_folder_path = parent_path.join(&entry_text);

                if new_folder_path.exists() || entry_text.is_empty() {
                    apply_button.set_sensitive(false);
                    entry.add_css_class("error");
                } else {
                    // Only allow creating valid folder names
                    apply_button.set_sensitive(true);
                    entry.remove_css_class("error");
                }
            }));

            connect_apply_button(&apply_button, &popover, &folder_name_entry, parent_path);

            popover.popup();
        } else {
            log::warn!("can't create new folder when there currently is no workspace selected.");
        }
    }));

    new_folder_action
}

fn create_folder_name_entry() -> Entry {
    Entry::builder()
        .placeholder_text(&gettext("Folder name"))
        .build()
}

fn create_dialog_title_label() -> Label {
    let label = Label::builder()
        .margin_bottom(12)
        .halign(Align::Center)
        .label(&gettext("New folder"))
        .width_chars(24)
        .ellipsize(pango::EllipsizeMode::End)
        .build();
    label.style_context().add_class("title-4");

    label
}

fn connect_apply_button(
    apply_button: &Button,
    popover: &Popover,
    entry: &Entry,
    parent_path: PathBuf,
) {
    apply_button.connect_clicked(clone!(@weak popover, @weak entry => move |_| {
        let new_folder_path = parent_path.join(entry.text().as_str());

        if new_folder_path.exists() {
            // Should have been caught earlier, but making sure
            log::error!("Couldn't create new folder wit name `{}`, it already exists.", entry.text().as_str());
        } else {
            if let Err(e) = fs_extra::dir::create(new_folder_path, false) {
                log::error!("Couldn't create folder: {}", e);
            }

            popover.popdown();
        }
    }));
}
