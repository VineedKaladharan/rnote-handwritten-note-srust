# config.rs
message('Configuring config.rs file')

global_conf = configuration_data()
global_conf.set_quoted('APP_ID', app_id)
global_conf.set_quoted('APP_IDPATH', app_idpath)
global_conf.set_quoted('APP_NAME', app_name)
global_conf.set_quoted('APP_NAME_CAPITALIZED', app_name_capitalized)
global_conf.set_quoted('APP_WEBSITE', app_website)
global_conf.set_quoted('APP_ISSUES_URL', app_issues_url)
global_conf.set_quoted('APP_SUPPORT_URL', app_support_url)
global_conf.set_quoted('APP_DONATE_URL', app_donate_url)
global_conf.set_quoted('APP_AUTHOR_NAME', app_author_name)
global_conf.set_quoted('APP_AUTHORS', app_authors)
global_conf.set_quoted('PKG_DATA_DIR', pkgdatadir)
global_conf.set_quoted('RESOURCES_FILE', pkgdatadir / 'resources.gresource')
global_conf.set_quoted('PROFILE', profile)
global_conf.set_quoted('VERSION', version)
global_conf.set_quoted('VERSION_SUFFIX', version_suffix)
global_conf.set_quoted('GETTEXT_PACKAGE', gettext_package)
global_conf.set_quoted('LOCALEDIR', localedir)
config = configure_file(
    input: 'config.rs.in',
    output: 'config.rs',
    configuration: global_conf,
)
# Copy the config.rs output to the source directory.
run_command(
    'cp',
    meson.project_build_root() / 'rnote-ui' / 'src' / 'config.rs',
    meson.project_source_root() / 'rnote-ui' / 'src' / 'config.rs',
    check: true,
)

rnote_ui_sources = files(
    'app/appactions.rs',
    'app/mod.rs',
    'appwindow/appsettings.rs',
    'appwindow/appwindowactions.rs',
    'appwindow/imexport.rs',
    'appwindow/mod.rs',
    'canvas/canvaslayout.rs',
    'canvas/input.rs',
    'canvas/mod.rs',
    'colorpicker/colorsetter.rs',
    'colorpicker/mod.rs',
    'penssidebar/brushpage.rs',
    'penssidebar/eraserpage.rs',
    'penssidebar/mod.rs',
    'penssidebar/selectorpage.rs',
    'penssidebar/shaperpage.rs',
    'penssidebar/toolspage.rs',
    'penssidebar/typewriterpage.rs',
    'settingspanel/mod.rs',
    'settingspanel/penshortcutmodels.rs',
    'settingspanel/penshortcutrow.rs',
    'workspacebrowser/filerow/action/duplicate.rs',
    'workspacebrowser/filerow/action/mod.rs',
    'workspacebrowser/filerow/action/open.rs',
    'workspacebrowser/filerow/action/rename.rs',
    'workspacebrowser/filerow/action/trash.rs',
    'workspacebrowser/filerow/mod.rs',
    'workspacebrowser/mod.rs',
    'workspacebrowser/widget_helper/entry_dialog.rs',
    'workspacebrowser/widget_helper/mod.rs',
    'workspacebrowser/workspace_action/create_folder.rs',
    'workspacebrowser/workspace_action/mod.rs',
    'workspacebrowser/workspacelist.rs',
    'workspacebrowser/workspacelistentry.rs',
    'workspacebrowser/workspacerow.rs',
    'dialogs/mod.rs',
    'dialogs/import.rs',
    'dialogs/export.rs',
    'appmenu.rs',
    'canvasmenu.rs',
    'config.rs',
    'globals.rs',
    'iconpicker.rs',
    'main.rs',
    'mainheader.rs',
    'unitentry.rs',
    'utils.rs',
)
