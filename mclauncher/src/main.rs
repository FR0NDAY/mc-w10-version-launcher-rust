#![windows_subsystem = "windows"]

use mclauncher_core::{
    build_download_filename, PackageType, Version, VersionDownloader, VersionList, VersionType,
    DEFAULT_VERSIONS_API_GDK, DEFAULT_VERSIONS_API_UWP,
};
use native_windows_derive as nwd;
use native_windows_gui as nwg;
use nwd::NwgUi;
use nwg::NativeUi;
use std::mem;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE};
use winreg::RegKey;
use windows::core::{HSTRING, PCWSTR};
use windows::Management::Deployment::PackageManager;
use windows::Win32::System::Services::{
    ChangeServiceConfigW, CloseServiceHandle, OpenSCManagerW, OpenServiceW, QueryServiceConfigW,
    QueryServiceStatusEx, ENUM_SERVICE_TYPE, QUERY_SERVICE_CONFIGW, SC_HANDLE, SC_MANAGER_CONNECT,
    SC_STATUS_PROCESS_INFO, SERVICE_AUTO_START, SERVICE_BOOT_START, SERVICE_CHANGE_CONFIG,
    SERVICE_CONTINUE_PENDING, SERVICE_DEMAND_START, SERVICE_DISABLED, SERVICE_ERROR,
    SERVICE_NO_CHANGE, SERVICE_PAUSED, SERVICE_PAUSE_PENDING, SERVICE_QUERY_CONFIG,
    SERVICE_QUERY_STATUS, SERVICE_RUNNING, SERVICE_START_PENDING, SERVICE_START_TYPE,
    SERVICE_STATUS_CURRENT_STATE, SERVICE_STATUS_PROCESS, SERVICE_STOPPED, SERVICE_STOP_PENDING,
    SERVICE_SYSTEM_START,
};
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};

#[derive(Default)]
struct LoadResult {
    versions: Vec<Version>,
    error: Option<String>,
    installed_summary: String,
    installed_details: String,
    dev_mode_enabled: bool,
    dev_mode_error: Option<String>,
    services: Vec<ServiceStatusInfo>,
    services_error: Option<String>,
}

#[derive(Default)]
struct ProgressState {
    current: u64,
    total: Option<u64>,
    done: bool,
    error: Option<String>,
}

#[derive(Default)]
struct ListMaps {
    release: Vec<usize>,
    beta: Vec<usize>,
    preview: Vec<usize>,
    imported: Vec<usize>,
}

#[derive(Default)]
struct InstalledInfo {
    summary: String,
    details: String,
}

#[derive(Default)]
struct DevModeInfo {
    enabled: bool,
    error: Option<String>,
}

#[derive(Clone)]
struct ServiceRequirement {
    name: &'static str,
    display: &'static str,
}

#[derive(Clone, Default)]
struct ServiceStatusInfo {
    display: String,
    state: String,
    start_type: String,
    is_disabled: bool,
    error: Option<String>,
}

#[derive(NwgUi)]
pub struct App {
    #[nwg_control(size: (1040, 660), position: (120, 120), title: "MCLauncher", flags: "WINDOW|VISIBLE")]
    #[nwg_events(OnInit: [App::init], OnWindowClose: [App::exit])]
    window: nwg::Window,

    #[nwg_layout(parent: window, spacing: 4, margin: [6, 6, 6, 6], max_row: Some(12), max_column: Some(2))]
    main_layout: nwg::GridLayout,

    #[nwg_control(parent: window, size: (10, 28))]
    #[nwg_layout_item(layout: main_layout, row: 0, col: 0, row_span: 1, col_span: 2)]
    toolbar: nwg::Frame,

    #[nwg_layout(parent: toolbar, spacing: 4, margin: [4, 4, 4, 4], max_column: Some(6))]
    toolbar_layout: nwg::GridLayout,

    #[nwg_control(parent: toolbar, text: "Refresh", size: (86, 22))]
    #[nwg_layout_item(layout: toolbar_layout, row: 0, col: 0)]
    #[nwg_events(OnButtonClick: [App::refresh_versions])]
    refresh_button: nwg::Button,

    #[nwg_control(parent: toolbar, text: "Download", enabled: false, size: (96, 22))]
    #[nwg_layout_item(layout: toolbar_layout, row: 0, col: 1)]
    #[nwg_events(OnButtonClick: [App::download_selected])]
    download_button: nwg::Button,

    #[nwg_control(parent: toolbar, text: "Search:")]
    #[nwg_layout_item(layout: toolbar_layout, row: 0, col: 2)]
    search_label: nwg::Label,

    #[nwg_control(parent: toolbar, size: (220, 22), focus: true)]
    #[nwg_layout_item(layout: toolbar_layout, row: 0, col: 3, col_span: 3)]
    #[nwg_events(OnTextInput: [App::search_changed])]
    search_input: nwg::TextInput,

    #[nwg_control(parent: window, flags: "VISIBLE", size: (720, 520))]
    #[nwg_layout_item(layout: main_layout, row: 1, col: 0, row_span: 10)]
    tabs: nwg::TabsContainer,

    #[nwg_control(parent: window, size: (260, 520))]
    #[nwg_layout_item(layout: main_layout, row: 1, col: 1, row_span: 10)]
    services_box: nwg::Frame,

    #[nwg_layout(parent: services_box, spacing: 4, margin: [6, 6, 6, 6], max_row: Some(3))]
    services_layout: nwg::GridLayout,

    #[nwg_control(parent: services_box, text: "Required services: (checking...)")]
    #[nwg_layout_item(layout: services_layout, row: 0, col: 0)]
    services_summary: nwg::Label,

    #[nwg_control(parent: services_box, list_style: nwg::ListViewStyle::Detailed, flags: "VISIBLE|TAB_STOP|SINGLE_SELECTION", ex_flags: nwg::ListViewExFlags::FULL_ROW_SELECT, background_color: [245, 245, 245], text_color: [0, 0, 0], size: (240, 460))]
    #[nwg_layout_item(layout: services_layout, row: 1, col: 0)]
    services_list: nwg::ListView,

    #[nwg_control(parent: services_box, text: "Set Disabled to Manual", size: (180, 22))]
    #[nwg_layout_item(layout: services_layout, row: 2, col: 0)]
    #[nwg_events(OnButtonClick: [App::fix_disabled_services])]
    services_fix_button: nwg::Button,

    #[nwg_control(parent: tabs, text: "Release")]
    release_tab: nwg::Tab,

    #[nwg_control(parent: tabs, text: "Beta")]
    beta_tab: nwg::Tab,

    #[nwg_control(parent: tabs, text: "Preview")]
    preview_tab: nwg::Tab,

    #[nwg_control(parent: tabs, text: "Imported")]
    imported_tab: nwg::Tab,

    #[nwg_layout(parent: release_tab, margin: [4, 4, 4, 4])]
    release_layout: nwg::GridLayout,

    #[nwg_control(parent: release_tab, list_style: nwg::ListViewStyle::Detailed, flags: "VISIBLE|TAB_STOP|SINGLE_SELECTION", ex_flags: nwg::ListViewExFlags::FULL_ROW_SELECT, background_color: [245, 245, 245], text_color: [0, 0, 0])]
    #[nwg_layout_item(layout: release_layout, row: 0, col: 0)]
    #[nwg_events(OnListViewItemChanged: [App::release_selected])]
    release_list: nwg::ListView,

    #[nwg_layout(parent: beta_tab, margin: [4, 4, 4, 4])]
    beta_layout: nwg::GridLayout,

    #[nwg_control(parent: beta_tab, list_style: nwg::ListViewStyle::Detailed, flags: "VISIBLE|TAB_STOP|SINGLE_SELECTION", ex_flags: nwg::ListViewExFlags::FULL_ROW_SELECT, background_color: [245, 245, 245], text_color: [0, 0, 0])]
    #[nwg_layout_item(layout: beta_layout, row: 0, col: 0)]
    #[nwg_events(OnListViewItemChanged: [App::beta_selected])]
    beta_list: nwg::ListView,

    #[nwg_layout(parent: preview_tab, margin: [4, 4, 4, 4])]
    preview_layout: nwg::GridLayout,

    #[nwg_control(parent: preview_tab, list_style: nwg::ListViewStyle::Detailed, flags: "VISIBLE|TAB_STOP|SINGLE_SELECTION", ex_flags: nwg::ListViewExFlags::FULL_ROW_SELECT, background_color: [245, 245, 245], text_color: [0, 0, 0])]
    #[nwg_layout_item(layout: preview_layout, row: 0, col: 0)]
    #[nwg_events(OnListViewItemChanged: [App::preview_selected])]
    preview_list: nwg::ListView,

    #[nwg_layout(parent: imported_tab, margin: [4, 4, 4, 4])]
    imported_layout: nwg::GridLayout,

    #[nwg_control(parent: imported_tab, list_style: nwg::ListViewStyle::Detailed, flags: "VISIBLE|TAB_STOP|SINGLE_SELECTION", ex_flags: nwg::ListViewExFlags::FULL_ROW_SELECT, background_color: [245, 245, 245], text_color: [0, 0, 0])]
    #[nwg_layout_item(layout: imported_layout, row: 0, col: 0)]
    #[nwg_events(OnListViewItemChanged: [App::imported_selected])]
    imported_list: nwg::ListView,

    #[nwg_control(parent: window, size: (10, 22))]
    #[nwg_layout_item(layout: main_layout, row: 11, col: 0, row_span: 1, col_span: 2)]
    status_bar: nwg::Frame,

    #[nwg_layout(parent: status_bar, spacing: 4, margin: [4, 2, 4, 2], max_column: Some(5))]
    status_layout: nwg::GridLayout,

    #[nwg_control(parent: status_bar, text: "Ready.")]
    #[nwg_layout_item(layout: status_layout, row: 0, col: 0)]
    status_label: nwg::Label,

    #[nwg_control(parent: status_bar, text: "Dev Mode: (checking...)" )]
    #[nwg_layout_item(layout: status_layout, row: 0, col: 1)]
    dev_mode_label: nwg::Label,

    #[nwg_control(parent: status_bar, text: "Enable Dev Mode", size: (140, 16))]
    #[nwg_layout_item(layout: status_layout, row: 0, col: 2)]
    #[nwg_events(OnButtonClick: [App::enable_dev_mode_clicked])]
    dev_mode_button: nwg::Button,

    #[nwg_control(parent: status_bar, text: "Installed: (checking...)", h_align: nwg::HTextAlign::Right)]
    #[nwg_layout_item(layout: status_layout, row: 0, col: 3)]
    #[nwg_events(OnLabelClick: [App::show_installed_info])]
    installed_label: nwg::Label,

    #[nwg_control(parent: status_bar, range: 0..100, pos: 0, size: (160, 16))]
    #[nwg_layout_item(layout: status_layout, row: 0, col: 4)]
    progress_bar: nwg::ProgressBar,

    #[nwg_control(parent: window)]
    #[nwg_events(OnNotice: [App::on_versions_loaded])]
    load_notice: nwg::Notice,

    #[nwg_control(parent: window)]
    #[nwg_events(OnNotice: [App::on_download_progress])]
    download_notice: nwg::Notice,

    #[nwg_control(parent: window)]
    #[nwg_events(OnNotice: [App::on_dev_mode_updated])]
    dev_mode_notice: nwg::Notice,

    #[nwg_control(parent: window)]
    #[nwg_events(OnNotice: [App::on_services_updated])]
    services_notice: nwg::Notice,

    load_result: Arc<Mutex<LoadResult>>,
    progress_state: Arc<Mutex<ProgressState>>,
    list_maps: Arc<Mutex<ListMaps>>,
    selected_index: Arc<Mutex<Option<usize>>>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            window: Default::default(),
            main_layout: Default::default(),
            toolbar: Default::default(),
            toolbar_layout: Default::default(),
            refresh_button: Default::default(),
            download_button: Default::default(),
            search_label: Default::default(),
            search_input: Default::default(),
            tabs: Default::default(),
            services_box: Default::default(),
            services_layout: Default::default(),
            services_summary: Default::default(),
            services_list: Default::default(),
            services_fix_button: Default::default(),
            release_tab: Default::default(),
            beta_tab: Default::default(),
            preview_tab: Default::default(),
            imported_tab: Default::default(),
            release_layout: Default::default(),
            release_list: Default::default(),
            beta_layout: Default::default(),
            beta_list: Default::default(),
            preview_layout: Default::default(),
            preview_list: Default::default(),
            imported_layout: Default::default(),
            imported_list: Default::default(),
            status_bar: Default::default(),
            status_layout: Default::default(),
            status_label: Default::default(),
            dev_mode_label: Default::default(),
            dev_mode_button: Default::default(),
            installed_label: Default::default(),
            progress_bar: Default::default(),
            load_notice: Default::default(),
            download_notice: Default::default(),
            dev_mode_notice: Default::default(),
            services_notice: Default::default(),
            load_result: Arc::new(Mutex::new(LoadResult::default())),
            progress_state: Arc::new(Mutex::new(ProgressState::default())),
            list_maps: Arc::new(Mutex::new(ListMaps::default())),
            selected_index: Arc::new(Mutex::new(None)),
        }
    }
}

impl App {
    fn init(&self) {
        self.setup_list(&self.release_list);
        self.setup_list(&self.beta_list);
        self.setup_list(&self.preview_list);
        self.setup_list(&self.imported_list);
        self.setup_services_list();

        self.progress_bar.set_pos(0);
        self.progress_bar.set_visible(false);
        self.dev_mode_button.set_visible(false);
        self.services_fix_button.set_visible(false);

        self.refresh_versions();
    }

    fn setup_list(&self, list: &nwg::ListView) {
        list.set_headers_enabled(true);
        list.insert_column(nwg::InsertListViewColumn {
            text: Some("Name".to_string()),
            width: Some(520),
            ..Default::default()
        });
        list.insert_column(nwg::InsertListViewColumn {
            text: Some("Package".to_string()),
            width: Some(70),
            ..Default::default()
        });
        list.insert_column(nwg::InsertListViewColumn {
            text: Some("New".to_string()),
            width: Some(40),
            ..Default::default()
        });
        list.insert_column(nwg::InsertListViewColumn {
            text: Some("Type".to_string()),
            width: Some(70),
            ..Default::default()
        });
    }

    fn setup_services_list(&self) {
        self.services_list.set_headers_enabled(true);
        self.services_list.insert_column(nwg::InsertListViewColumn {
            text: Some("Service".to_string()),
            width: Some(140),
            ..Default::default()
        });
        self.services_list.insert_column(nwg::InsertListViewColumn {
            text: Some("Status".to_string()),
            width: Some(70),
            ..Default::default()
        });
        self.services_list.insert_column(nwg::InsertListViewColumn {
            text: Some("Start".to_string()),
            width: Some(70),
            ..Default::default()
        });
    }

    fn refresh_versions(&self) {
        self.status_label.set_text("Refreshing version list...");
        self.refresh_button.set_enabled(false);
        self.download_button.set_enabled(false);

        let notice = self.load_notice.sender();
        let load_result = self.load_result.clone();

        thread::spawn(move || {
            let result = load_versions(true);
            let installed = detect_installed_info();
            let dev_mode = detect_dev_mode();
            let (services, services_error) = detect_services();
            {
                let mut state = load_result.lock().unwrap();
                state.error = result.as_ref().err().map(|e| e.to_string());
                state.versions = result.unwrap_or_default();
                state.installed_summary = installed.summary;
                state.installed_details = installed.details;
                state.dev_mode_enabled = dev_mode.enabled;
                state.dev_mode_error = dev_mode.error;
                state.services = services;
                state.services_error = services_error;
            }
            notice.notice();
        });
    }

    fn on_versions_loaded(&self) {
        self.refresh_button.set_enabled(true);

        let mut load_result = self.load_result.lock().unwrap();
        let load_error = load_result.error.take();

        if !load_result.installed_summary.is_empty() {
            self.installed_label
                .set_text(&load_result.installed_summary);
        }
        self.update_dev_mode_ui(load_result.dev_mode_enabled, load_result.dev_mode_error.clone());
        self.update_services_ui(&load_result.services, load_result.services_error.as_deref());

        if let Some(err) = load_error {
            self.status_label
                .set_text(&format!("Failed to load versions: {}", err));
            nwg::simple_message("Load error", &err);
            return;
        }

        self.populate_lists(&load_result.versions);
        self.status_label.set_text("Version list updated.");
    }

    fn search_changed(&self) {
        let versions = self.load_result.lock().unwrap().versions.clone();
        self.populate_lists(&versions);
    }

    fn populate_lists(&self, versions: &[Version]) {
        self.release_list.clear();
        self.beta_list.clear();
        self.preview_list.clear();
        self.imported_list.clear();

        let mut maps = self.list_maps.lock().unwrap();
        maps.release.clear();
        maps.beta.clear();
        maps.preview.clear();
        maps.imported.clear();

        let query = self.search_input.text().trim().to_lowercase();
        for (idx, version) in versions.iter().enumerate() {
            if !query.is_empty() {
                let name = version.name.to_lowercase();
                if !name.contains(&query) {
                    continue;
                }
            }
            let (list, map) = match version.version_type {
                VersionType::Release => (&self.release_list, &mut maps.release),
                VersionType::Beta => (&self.beta_list, &mut maps.beta),
                VersionType::Preview => (&self.preview_list, &mut maps.preview),
                VersionType::Imported => (&self.imported_list, &mut maps.imported),
            };

            let pkg = match version.package_type {
                PackageType::Uwp => "UWP",
                PackageType::Gdk => "GDK",
            };
            let type_text = match version.version_type {
                VersionType::Release => "Release",
                VersionType::Beta => "Beta",
                VersionType::Preview => "Preview",
                VersionType::Imported => "Imported",
            };
            let items = vec![
                version.name.clone(),
                pkg.to_string(),
                if version.is_new { "yes".to_string() } else { "".to_string() },
                type_text.to_string(),
            ];

            list.insert_items_row(Some(list.len() as i32), &items);
            map.push(idx);
        }

        *self.selected_index.lock().unwrap() = None;
        self.download_button.set_enabled(false);
    }

    fn release_selected(&self) {
        self.select_from_list(&self.release_list, |maps| &maps.release);
    }

    fn beta_selected(&self) {
        self.select_from_list(&self.beta_list, |maps| &maps.beta);
    }

    fn preview_selected(&self) {
        self.select_from_list(&self.preview_list, |maps| &maps.preview);
    }

    fn imported_selected(&self) {
        self.select_from_list(&self.imported_list, |maps| &maps.imported);
    }

    fn select_from_list<F>(&self, list: &nwg::ListView, map_fn: F)
    where
        F: FnOnce(&ListMaps) -> &Vec<usize>,
    {
        let selected = list.selected_item();
        let maps = self.list_maps.lock().unwrap();
        let map = map_fn(&maps);
        let version_index = selected.and_then(|row| map.get(row).copied());
        *self.selected_index.lock().unwrap() = version_index;
        self.download_button
            .set_enabled(version_index.is_some());
    }

    fn download_selected(&self) {
        let selected_index = *self.selected_index.lock().unwrap();
        if selected_index.is_none() {
            return;
        }

        let index = selected_index.unwrap();
        let versions = self.load_result.lock().unwrap().versions.clone();
        let version = match versions.get(index) {
            Some(v) => v.clone(),
            None => {
                nwg::simple_message("Error", "Selected version not found.");
                return;
            }
        };

        let output = PathBuf::from(build_download_filename(
            &version.name,
            version.version_type,
            version.package_type,
        ));

        if version.package_type == PackageType::Uwp {
            nwg::simple_message(
                "Token required",
                "UWP downloads require a Microsoft Store token. This UI does not fetch it yet.",
            );
            return;
        }

        self.status_label
            .set_text(&format!("Downloading to {}...", output.display()));
        self.progress_bar.set_visible(true);
        self.progress_bar.set_pos(0);
        self.download_button.set_enabled(false);
        self.refresh_button.set_enabled(false);

        let notice = Arc::new(self.download_notice.sender());
        let progress_state = self.progress_state.clone();

        thread::spawn(move || {
            let downloader = match VersionDownloader::new() {
                Ok(d) => d,
                Err(err) => {
                    {
                        let mut state = progress_state.lock().unwrap();
                        state.error = Some(err.to_string());
                        state.done = true;
                    }
                    notice.notice();
                    return;
                }
            };

            let mut last_emit = Instant::now();
            let progress_state_clone = progress_state.clone();
            let notice_clone = notice.clone();

            let task = async move {
                let progress = move |current: u64, total: Option<u64>| {
                    let mut send_notice = false;
                    {
                        let mut state = progress_state_clone.lock().unwrap();
                        state.current = current;
                        state.total = total;
                        if last_emit.elapsed() >= Duration::from_millis(200) {
                            last_emit = Instant::now();
                            send_notice = true;
                        }
                    }
                    if send_notice {
                        notice_clone.notice();
                    }
                };
                match version.package_type {
                    PackageType::Uwp => {
                        downloader.download_appx(&version.uuid, "1", &output, progress).await
                    }
                    PackageType::Gdk => {
                        downloader
                            .download_msixvc(&version.download_urls, &output, progress)
                            .await
                    }
                }
            };

            let result = tokio::runtime::Runtime::new().unwrap().block_on(task);
            {
                let mut state = progress_state.lock().unwrap();
                if let Err(err) = result {
                    state.error = Some(err.to_string());
                }
                state.done = true;
            }
            notice.notice();
        });
    }

    fn on_download_progress(&self) {
        let mut state = self.progress_state.lock().unwrap();
        if let Some(total) = state.total {
            let percent = if total == 0 {
                0
            } else {
                ((state.current as f64 / total as f64) * 100.0).round() as u32
            };
            self.progress_bar.set_pos(percent);
            self.status_label
                .set_text(&format!("Downloading... {}%", percent));
        } else {
            self.status_label.set_text("Downloading...");
        }

        if state.done {
            self.progress_bar.set_visible(false);
            self.refresh_button.set_enabled(true);
            self.download_button.set_enabled(true);
            if let Some(err) = state.error.take() {
                self.status_label.set_text("Download failed.");
                nwg::simple_message("Download failed", &err);
            } else {
                self.status_label.set_text("Download complete.");
            }
            *state = ProgressState::default();
        }
    }

    fn update_dev_mode_ui(&self, enabled: bool, error: Option<String>) {
        if error.is_some() {
            self.dev_mode_label.set_text("Dev Mode: Unknown");
            self.dev_mode_button.set_visible(true);
            self.dev_mode_button.set_enabled(true);
            return;
        }

        if enabled {
            self.dev_mode_label.set_text("Dev Mode: On");
            self.dev_mode_button.set_visible(false);
        } else {
            self.dev_mode_label.set_text("Dev Mode: Off");
            self.dev_mode_button.set_visible(true);
            self.dev_mode_button.set_enabled(true);
        }
    }

    fn on_dev_mode_updated(&self) {
        let mut load_result = self.load_result.lock().unwrap();
        self.update_dev_mode_ui(load_result.dev_mode_enabled, load_result.dev_mode_error.clone());
        if let Some(err) = load_result.dev_mode_error.take() {
            self.status_label.set_text("Developer Mode update failed.");
            nwg::simple_message("Developer Mode", &err);
        } else {
            self.status_label.set_text("Developer Mode updated.");
        }
    }

    fn enable_dev_mode_clicked(&self) {
        self.dev_mode_button.set_enabled(false);
        self.status_label.set_text("Enabling Developer Mode...");

        let notice = self.dev_mode_notice.sender();
        let load_result = self.load_result.clone();

        thread::spawn(move || {
            let enable_result = set_dev_mode_enabled();
            let dev_mode = detect_dev_mode();
            let mut state = load_result.lock().unwrap();
            state.dev_mode_enabled = dev_mode.enabled;
            state.dev_mode_error = if dev_mode.enabled {
                None
            } else {
                dev_mode.error.or_else(|| enable_result.err())
            };
            notice.notice();
        });
    }

    fn update_services_ui(&self, services: &[ServiceStatusInfo], error: Option<&str>) {
        self.services_list.clear();
        let mut disabled = 0usize;

        for service in services {
            let status_text = if service.error.is_some() {
                "Error".to_string()
            } else {
                service.state.clone()
            };
            let start_text = if service.error.is_some() {
                "-".to_string()
            } else {
                service.start_type.clone()
            };
            if service.is_disabled {
                disabled += 1;
            }

            self.services_list.insert_items_row(
                Some(self.services_list.len() as i32),
                &[service.display.clone(), status_text, start_text],
            );
        }

        let summary = if let Some(err) = error {
            format!("Required services: error ({})", err)
        } else if services.is_empty() {
            "Required services: none".to_string()
        } else if disabled == 0 {
            format!("Required services: OK ({}/{})", services.len(), services.len())
        } else {
            format!("Required services disabled: {}/{}", disabled, services.len())
        };
        self.services_summary.set_text(&summary);
        self.services_fix_button.set_visible(disabled > 0);
        self.services_fix_button.set_enabled(disabled > 0);
    }

    fn on_services_updated(&self) {
        let mut load_result = self.load_result.lock().unwrap();
        self.update_services_ui(
            &load_result.services,
            load_result.services_error.as_deref(),
        );
        if let Some(err) = load_result.services_error.take() {
            nwg::simple_message("Services", &err);
        }
        self.status_label.set_text("Services updated.");
    }

    fn fix_disabled_services(&self) {
        self.services_fix_button.set_enabled(false);
        self.status_label.set_text("Updating services...");

        let notice = self.services_notice.sender();
        let load_result = self.load_result.clone();

        thread::spawn(move || {
            let action_error = set_disabled_services_manual();
            let (services, services_error) = detect_services();
            let mut state = load_result.lock().unwrap();
            state.services = services;
            state.services_error = action_error.or(services_error);
            notice.notice();
        });
    }

    fn show_installed_info(&self) {
        let load_result = self.load_result.lock().unwrap();
        let details = if load_result.installed_details.is_empty() {
            "No installed packages detected.".to_string()
        } else {
            load_result.installed_details.clone()
        };
        nwg::simple_message("Installed Minecraft packages", &details);
    }

    fn exit(&self) {
        nwg::stop_thread_dispatch();
    }
}

fn load_versions(refresh: bool) -> Result<Vec<Version>, mclauncher_core::Error> {
    let runtime = tokio::runtime::Runtime::new().map_err(mclauncher_core::Error::from)?;
    runtime.block_on(async move {
        let mut list = VersionList::new(
            "versions_uwp.json",
            "versions_gdk.json",
            "imported_versions",
            DEFAULT_VERSIONS_API_UWP.to_string(),
            DEFAULT_VERSIONS_API_GDK.to_string(),
        )?;

        list.prepare_for_reload();
        list.load_from_cache_gdk().await?;
        list.load_from_cache_uwp().await?;

        if refresh {
            list.prepare_for_reload();
            list.download_versions_gdk().await?;
            list.download_versions_uwp().await?;
        }

        list.load_imported()?;
        Ok(list.versions().to_vec())
    })
}

fn main() {
    nwg::init().expect("Failed to init Native Windows GUI");
    nwg::Font::set_global_family("Segoe UI").expect("Failed to set default font");

    let _app = App::build_ui(Default::default()).expect("Failed to build UI");
    nwg::dispatch_thread_events();
}

fn detect_dev_mode() -> DevModeInfo {
    let key_path = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\AppModelUnlock";
    let key = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(key_path, KEY_READ);
    match key {
        Ok(key) => {
            let allow: u32 = key.get_value("AllowDevelopmentWithoutDevLicense").unwrap_or(0);
            DevModeInfo {
                enabled: allow == 1,
                error: None,
            }
        }
        Err(err) => DevModeInfo {
            enabled: false,
            error: Some(format!("Registry read failed: {}", err)),
        },
    }
}

fn set_dev_mode_enabled() -> Result<(), String> {
    let key_path = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\AppModelUnlock";
    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(key_path, KEY_READ | KEY_WRITE)
        .map_err(|err| format!("Registry write failed: {}", err))?;

    key.set_value("AllowDevelopmentWithoutDevLicense", &1u32)
        .map_err(|err| format!("Failed to enable Developer Mode: {}", err))?;
    key.set_value("AllowAllTrustedApps", &1u32)
        .map_err(|err| format!("Failed to enable trusted apps: {}", err))?;
    Ok(())
}

fn detect_installed_info() -> InstalledInfo {
    match detect_installed_packages() {
        Ok(info) => info,
        Err(err) => InstalledInfo {
            summary: "Installed: (error)".to_string(),
            details: format!("Failed to query installed packages: {}", err),
        },
    }
}

fn detect_installed_packages() -> Result<InstalledInfo, windows::core::Error> {
    let _ro = RoInit::init()?;
    let manager = PackageManager::new()?;
    let release_best = find_best_for_family(&manager, mclauncher_core::PackageFamilies::MINECRAFT)?;
    let preview_best =
        find_best_for_family(&manager, mclauncher_core::PackageFamilies::MINECRAFT_PREVIEW)?;

    let release_summary = match &release_best {
        Some((_, version, _, _)) => format!("Release {} (registered)", version),
        None => "Release not installed".to_string(),
    };
    let preview_summary = match &preview_best {
        Some((_, version, _, _)) => format!("Preview {} (registered)", version),
        None => "Preview not installed".to_string(),
    };

    let summary = format!("Installed: {} | {}", release_summary, preview_summary);

    let mut details = String::new();
    details.push_str("Release\n");
    match &release_best {
        Some((_, version, family, path)) => {
            details.push_str(&format!("  Status: registered\n  Version: {}\n", version));
            details.push_str(&format!(
                "  Package family: {}\n  Install path: {}\n\n",
                family, path
            ));
        }
        None => {
            details.push_str("  Status: not installed\n\n");
        }
    }

    details.push_str("Preview\n");
    match &preview_best {
        Some((_, version, family, path)) => {
            details.push_str(&format!("  Status: registered\n  Version: {}\n", version));
            details.push_str(&format!(
                "  Package family: {}\n  Install path: {}\n",
                family, path
            ));
        }
        None => {
            details.push_str("  Status: not installed\n");
        }
    }

    Ok(InstalledInfo { summary, details })
}

fn find_best_for_family(
    manager: &PackageManager,
    family: &str,
) -> Result<Option<(u64, String, String, String)>, windows::core::Error> {
    let packages = manager.FindPackagesByPackageFamilyName(&HSTRING::from(family))?;
    let mut best: Option<(u64, String, String, String)> = None;

    for pkg in packages {
        let id = pkg.Id()?;
        let version = id.Version()?;
        let version_str = format!(
            "{}.{}.{}.{}",
            version.Major, version.Minor, version.Build, version.Revision
        );
        let version_key = ((version.Major as u64) << 48)
            | ((version.Minor as u64) << 32)
            | ((version.Build as u64) << 16)
            | (version.Revision as u64);
        let path = pkg
            .InstalledLocation()
            .ok()
            .and_then(|loc| loc.Path().ok())
            .map(|p| p.to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let replace = best
            .as_ref()
            .map(|(key, _, _, _)| version_key > *key)
            .unwrap_or(true);
        if replace {
            best = Some((version_key, version_str, family.to_string(), path));
        }
    }

    Ok(best)
}

const REQUIRED_SERVICES: &[ServiceRequirement] = &[
    ServiceRequirement {
        name: "AppXSVC",
        display: "AppX Deployment Service",
    },
    ServiceRequirement {
        name: "ClipSVC",
        display: "Client License Service",
    },
    ServiceRequirement {
        name: "InstallService",
        display: "Microsoft Store Install Service",
    },
    ServiceRequirement {
        name: "LicenseManager",
        display: "Windows License Manager Service",
    },
    ServiceRequirement {
        name: "StateRepository",
        display: "State Repository Service",
    },
    ServiceRequirement {
        name: "AppReadiness",
        display: "App Readiness",
    },
    ServiceRequirement {
        name: "BITS",
        display: "Background Intelligent Transfer Service",
    },
    ServiceRequirement {
        name: "wuauserv",
        display: "Windows Update",
    },
    ServiceRequirement {
        name: "WaaSMedicSvc",
        display: "Windows Update Medic Service",
    },
    ServiceRequirement {
        name: "DoSvc",
        display: "Delivery Optimization",
    },
    ServiceRequirement {
        name: "UsoSvc",
        display: "Update Orchestrator Service",
    },
    ServiceRequirement {
        name: "XblAuthManager",
        display: "Xbox Live Auth Manager",
    },
    ServiceRequirement {
        name: "XblGameSave",
        display: "Xbox Live Game Save",
    },
    ServiceRequirement {
        name: "XboxNetApiSvc",
        display: "Xbox Live Networking Service",
    },
    ServiceRequirement {
        name: "GamingServices",
        display: "Gaming Services",
    },
];

struct ServiceHandle(SC_HANDLE);

impl ServiceHandle {
    fn raw(&self) -> SC_HANDLE {
        self.0
    }
}

impl Drop for ServiceHandle {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_invalid() {
                let _ = CloseServiceHandle(self.0);
            }
        }
    }
}

struct RoInit;

impl RoInit {
    fn init() -> Result<Self, windows::core::Error> {
        unsafe {
            RoInitialize(RO_INIT_MULTITHREADED)?;
        }
        Ok(Self)
    }
}

impl Drop for RoInit {
    fn drop(&mut self) {
        unsafe {
            RoUninitialize();
        }
    }
}

fn detect_services() -> (Vec<ServiceStatusInfo>, Option<String>) {
    match open_scm_handle() {
        Ok(scm) => {
            let mut services = Vec::new();
            for req in REQUIRED_SERVICES {
                services.push(query_service_info(&scm, req));
            }
            (services, None)
        }
        Err(err) => {
            let services = REQUIRED_SERVICES
                .iter()
                .map(|req| ServiceStatusInfo {
                    display: req.display.to_string(),
                    state: "Unknown".to_string(),
                    start_type: "Unknown".to_string(),
                    is_disabled: false,
                    error: Some(err.clone()),
                })
                .collect();
            (services, Some(err))
        }
    }
}

fn query_service_info(scm: &ServiceHandle, req: &ServiceRequirement) -> ServiceStatusInfo {
    let mut info = ServiceStatusInfo {
        display: req.display.to_string(),
        state: "Unknown".to_string(),
        start_type: "Unknown".to_string(),
        is_disabled: false,
        error: None,
    };

    let service = match open_service_handle(
        scm,
        req.name,
        SERVICE_QUERY_STATUS | SERVICE_QUERY_CONFIG,
    ) {
        Ok(service) => service,
        Err(err) => {
            info.error = Some(err);
            info.state = "Missing".to_string();
            return info;
        }
    };

    let mut bytes_needed = 0u32;
    let mut status_buf = vec![0u8; mem::size_of::<SERVICE_STATUS_PROCESS>()];
    if let Err(err) = unsafe {
        QueryServiceStatusEx(
            service.raw(),
            SC_STATUS_PROCESS_INFO,
            Some(status_buf.as_mut_slice()),
            &mut bytes_needed,
        )
    } {
        info.error = Some(format!("Status query failed: {}", err));
        return info;
    }

    let status: SERVICE_STATUS_PROCESS =
        unsafe { std::ptr::read_unaligned(status_buf.as_ptr() as *const _) };
    info.state = service_state_text(status.dwCurrentState).to_string();

    let mut config_bytes = 0u32;
    unsafe {
        let _ = QueryServiceConfigW(service.raw(), None, 0, &mut config_bytes);
    }
    if config_bytes == 0 {
        info.error = Some("Config query failed".to_string());
        return info;
    }

    let mut config_buf = vec![0u8; config_bytes as usize];
    if let Err(err) = unsafe {
        QueryServiceConfigW(
            service.raw(),
            Some(config_buf.as_mut_ptr() as *mut QUERY_SERVICE_CONFIGW),
            config_bytes,
            &mut config_bytes,
        )
    } {
        info.error = Some(format!("Config query failed: {}", err));
        return info;
    }

    let config: QUERY_SERVICE_CONFIGW =
        unsafe { std::ptr::read_unaligned(config_buf.as_ptr() as *const _) };
    info.start_type = service_start_type_text(config.dwStartType).to_string();
    info.is_disabled = config.dwStartType == SERVICE_DISABLED;

    info
}

fn set_disabled_services_manual() -> Option<String> {
    let scm = match open_scm_handle() {
        Ok(scm) => scm,
        Err(err) => return Some(err),
    };

    let mut errors = Vec::new();
    for req in REQUIRED_SERVICES {
        let info = query_service_info(&scm, req);
        if let Some(err) = info.error {
            errors.push(format!("{}: {}", req.name, err));
            continue;
        }
        if info.is_disabled {
            if let Err(err) = set_service_manual(&scm, req.name) {
                errors.push(format!("{}: {}", req.name, err));
            }
        }
    }

    if errors.is_empty() {
        None
    } else {
        Some(errors.join("\n"))
    }
}

fn set_service_manual(scm: &ServiceHandle, name: &str) -> Result<(), String> {
    let service = open_service_handle(scm, name, SERVICE_CHANGE_CONFIG)?;
    unsafe {
        ChangeServiceConfigW(
            service.raw(),
            ENUM_SERVICE_TYPE(SERVICE_NO_CHANGE),
            SERVICE_DEMAND_START,
            SERVICE_ERROR(SERVICE_NO_CHANGE),
            PCWSTR::null(),
            PCWSTR::null(),
            None,
            PCWSTR::null(),
            PCWSTR::null(),
            PCWSTR::null(),
            PCWSTR::null(),
        )
        .map_err(|err| format!("ChangeServiceConfig failed: {}", err))
    }
}

fn open_scm_handle() -> Result<ServiceHandle, String> {
    unsafe {
        OpenSCManagerW(None, None, SC_MANAGER_CONNECT)
            .map(ServiceHandle)
            .map_err(|err| format!("OpenSCManager failed: {}", err))
    }
}

fn open_service_handle(
    scm: &ServiceHandle,
    name: &str,
    access: u32,
) -> Result<ServiceHandle, String> {
    let name_w = to_wide(name);
    unsafe {
        OpenServiceW(scm.raw(), PCWSTR::from_raw(name_w.as_ptr()), access)
            .map(ServiceHandle)
            .map_err(|err| format!("OpenService failed: {}", err))
    }
}

fn to_wide(value: &str) -> Vec<u16> {
    let mut wide: Vec<u16> = value.encode_utf16().collect();
    wide.push(0);
    wide
}

fn service_state_text(state: SERVICE_STATUS_CURRENT_STATE) -> &'static str {
    if state == SERVICE_RUNNING {
        "Running"
    } else if state == SERVICE_STOPPED {
        "Stopped"
    } else if state == SERVICE_START_PENDING {
        "Start pending"
    } else if state == SERVICE_STOP_PENDING {
        "Stop pending"
    } else if state == SERVICE_CONTINUE_PENDING {
        "Continue pending"
    } else if state == SERVICE_PAUSE_PENDING {
        "Pause pending"
    } else if state == SERVICE_PAUSED {
        "Paused"
    } else {
        "Unknown"
    }
}

fn service_start_type_text(start_type: SERVICE_START_TYPE) -> &'static str {
    if start_type == SERVICE_AUTO_START {
        "Automatic"
    } else if start_type == SERVICE_DEMAND_START {
        "Manual"
    } else if start_type == SERVICE_DISABLED {
        "Disabled"
    } else if start_type == SERVICE_BOOT_START {
        "Boot"
    } else if start_type == SERVICE_SYSTEM_START {
        "System"
    } else {
        "Unknown"
    }
}
