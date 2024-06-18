use azure_storage::prelude::*;
use azure_storage_blobs::prelude::*;
use druid::{
    theme,
    widget::{Button, Flex, Label, TextBox},
    AppDelegate, AppLauncher, Color, Command, Data, DelegateCtx, Env, Handled, Lens, Selector,
    Target, Widget, WidgetExt, WindowDesc,
};
use futures::future::try_join_all;
use std::fs;
use std::io::Read;
use std::path::Path;

#[derive(Clone, Lens)]
struct Uploader {
    container: String,
    folder_path: String,
    upload_folder: String,
    storage_account: String,
    storage_account_key: String,
    info: String,
    error: String,
}

const UPDATE_PROGRESS: Selector<f64> = Selector::new("uploader.update-progress");
const UPDATE_ERROR: Selector<String> = Selector::new("uploader.update-error");
const UPDATE_INFO: Selector<String> = Selector::new("uploader.update-info");
const UPDATE_FILE_NAME: Selector<String> = Selector::new("uploader.update-file-name");
const UPDATE_BLOB_NAME: Selector<String> = Selector::new("uploader.update-blob-name");

fn ui_builder() -> impl Widget<Uploader> {
    let info_label = Label::new(|data: &Uploader, _env: &Env| format!("{}", data.info));

    let error_label = Label::new(|data: &Uploader, _env: &Env| format!("{}", data.error))
        .with_text_color(Color::rgb8(255, 0, 0));

    let container_input = Flex::row()
        .with_child(Label::new("Container:"))
        .with_flex_child(TextBox::new().lens(Uploader::container), 1.0);

    let folder_path_input = Flex::row()
        .with_child(Label::new("Folder Path:"))
        .with_flex_child(TextBox::new().lens(Uploader::folder_path), 1.0);

    let upload_folder_input = Flex::row()
        .with_child(Label::new("Upload Folder:"))
        .with_flex_child(TextBox::new().lens(Uploader::upload_folder), 1.0);

    let storage_account_input = Flex::row()
        .with_child(Label::new("Storage Account:"))
        .with_flex_child(TextBox::new().lens(Uploader::storage_account), 1.0);

    let storage_account_key_input = Flex::row()
        .with_child(Label::new("Storage Account Key:"))
        .with_flex_child(TextBox::new().lens(Uploader::storage_account_key), 1.0);

    let upload_button = Flex::row().with_child(Button::new("Upload").on_click(
        move |ctx, data: &mut Uploader, _env| {
            let container = data.container.clone();
            let folder_path = data.folder_path.clone();
            let upload_folder = data.upload_folder.clone();
            let account = data.storage_account.clone();
            let access_key = data.storage_account_key.clone();
            let ext_event_sink = ctx.get_external_handle();

            // Check if all necessary inputs are provided
            if container.is_empty() {
                data.error = "Container is required.".to_string();
                return;
            }
            if folder_path.is_empty() {
                data.error = "Folder Path is required.".to_string();
                return;
            }
            if upload_folder.is_empty() {
                data.error = "Upload Folder is required.".to_string();
                return;
            }
            if account.is_empty() {
                data.error = "Storage Account is required.".to_string();
                return;
            }
            if access_key.is_empty() {
                data.error = "Storage Account Key is required.".to_string();
                return;
            }

            // Check if the path exists
            if !Path::new(&folder_path).exists() {
                data.error = "Folder path not found.".to_string();
                return;
            }

            data.info = format!(
                "Uploading files from {} to {}/{} in container {}...",
                folder_path, account, upload_folder, container
            );
            data.error.clear(); // Clear previous errors

            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
                rt.block_on(async move {
                    let storage_credentials =
                        StorageCredentials::access_key(account.clone(), access_key.clone());
                    let blob_service_client =
                        BlobServiceClient::new(account.clone(), storage_credentials);

                    let entries: Vec<_> = match fs::read_dir(Path::new(&folder_path)) {
                        Ok(entries) => entries.collect(),
                        Err(err) => {
                            ext_event_sink
                                .submit_command(
                                    UPDATE_ERROR,
                                    format!("Directory read error: {}", err),
                                    Target::Auto,
                                )
                                .unwrap();
                            return;
                        }
                    };

                    let total_files = entries.len() as u64;
                    let mut upload_futures = vec![];

                    for (idx, entry) in entries.into_iter().enumerate() {
                        let entry = match entry {
                            Ok(entry) => entry,
                            Err(err) => {
                                ext_event_sink
                                    .submit_command(
                                        UPDATE_ERROR,
                                        format!("Failed to read entry: {}", err),
                                        Target::Auto,
                                    )
                                    .unwrap();
                                continue;
                            }
                        };

                        let path = entry.path();
                        if path.is_file() {
                            let file_name = match path.file_name() {
                                Some(name) => match name.to_str() {
                                    Some(name_str) => name_str.to_string(),
                                    None => {
                                        ext_event_sink
                                            .submit_command(
                                                UPDATE_ERROR,
                                                "Failed to convert file name to string".to_string(),
                                                Target::Auto,
                                            )
                                            .unwrap();
                                        continue;
                                    }
                                },
                                None => {
                                    ext_event_sink
                                        .submit_command(
                                            UPDATE_ERROR,
                                            "Failed to get file name".to_string(),
                                            Target::Auto,
                                        )
                                        .unwrap();
                                    continue;
                                }
                            };

                            let blob_name = format!("{}/{}", upload_folder, file_name);

                            ext_event_sink
                                .submit_command(UPDATE_FILE_NAME, file_name.clone(), Target::Auto)
                                .unwrap();

                            ext_event_sink
                                .submit_command(UPDATE_BLOB_NAME, blob_name.clone(), Target::Auto)
                                .unwrap();

                            let mut file = match fs::File::open(&path) {
                                Ok(file) => file,
                                Err(err) => {
                                    ext_event_sink
                                        .submit_command(
                                            UPDATE_ERROR,
                                            format!("Failed to open file {}: {}", file_name, err),
                                            Target::Auto,
                                        )
                                        .unwrap();
                                    continue;
                                }
                            };

                            let mut data = Vec::new();
                            if let Err(err) = file.read_to_end(&mut data) {
                                ext_event_sink
                                    .submit_command(
                                        UPDATE_ERROR,
                                        format!("Failed to read file {}: {}", file_name, err),
                                        Target::Auto,
                                    )
                                    .unwrap();
                                continue;
                            }

                            let blob_client = blob_service_client
                                .container_client(&container)
                                .blob_client(&blob_name);

                            let task = blob_client.put_block_blob(data).into_future();
                            upload_futures.push(task);

                            ext_event_sink
                                .submit_command(
                                    UPDATE_PROGRESS,
                                    ((idx as f64 + 1.0) / total_files as f64) * 100.0,
                                    Target::Auto,
                                )
                                .unwrap();
                        }
                    }

                    // Allow all blobs to upload.
                    if upload_futures.is_empty() {
                        ext_event_sink
                            .submit_command(
                                UPDATE_INFO,
                                "No files to upload.".to_string(),
                                Target::Auto,
                            )
                            .unwrap();
                    } else {
                        // Allow all blobs to upload.
                        match try_join_all(upload_futures).await {
                            Ok(_) => {
                                ext_event_sink
                                    .submit_command(UPDATE_PROGRESS, 100.0, Target::Auto)
                                    .unwrap();
                                ext_event_sink
                                    .submit_command(
                                        UPDATE_INFO,
                                        "All files uploaded successfully!".to_string(),
                                        Target::Auto,
                                    )
                                    .unwrap();
                            }
                            Err(err) => {
                                ext_event_sink
                                    .submit_command(
                                        UPDATE_ERROR,
                                        format!("Failed to upload files: {:?}", err),
                                        Target::Auto,
                                    )
                                    .unwrap();
                            }
                        }
                    }
                });
            });
        },
    ));

    Flex::column()
        .with_child(container_input)
        .with_spacer(10.0)
        .with_child(folder_path_input)
        .with_spacer(10.0)
        .with_child(upload_folder_input)
        .with_spacer(10.0)
        .with_child(storage_account_input)
        .with_spacer(10.0)
        .with_child(storage_account_key_input)
        .with_spacer(10.0)
        .with_child(upload_button)
        .with_spacer(10.0)
        .with_child(info_label)
        .with_child(error_label)
        .padding(10.0)
}

fn main() {
    #[cfg(target_os = "windows")]
    {
        use winapi::um::wincon::FreeConsole;
        unsafe {
            FreeConsole();
        }
    }
    let main_window = WindowDesc::new(ui_builder()).title("Azure Storage Uploader");

    let initial_state = Uploader {
        container: String::new(),
        folder_path: String::new(),
        upload_folder: String::new(),
        storage_account: String::new(),
        storage_account_key: String::new(),
        info: String::new(),
        error: String::new(),
    };

    AppLauncher::with_window(main_window)
        .configure_env(|env, _| {
            env.set(theme::TEXT_SIZE_NORMAL, 16.0);
            // Set the background color to black
            env.set(theme::BACKGROUND_LIGHT, Color::rgb8(0, 0, 0));
            // Set the background color to black
            env.set(theme::BACKGROUND_DARK, Color::rgb8(0, 0, 0));
            // Set the primary light color to blue
            env.set(theme::PRIMARY_LIGHT, Color::rgb8(0, 0, 255));
            // Set the primary dark color to red
            env.set(theme::PRIMARY_DARK, Color::rgb8(255, 0, 0));
        })
        .delegate(Delegate)
        .launch(initial_state)
        .expect("Failed to launch application");
}

struct Delegate;

impl AppDelegate<Uploader> for Delegate {
    fn command(
        &mut self,
        _: &mut DelegateCtx,
        _: Target,
        cmd: &Command,
        data: &mut Uploader,
        _: &Env,
    ) -> Handled {
        if let Some(progress) = cmd.get(UPDATE_PROGRESS) {
            data.info.push_str(&format!("\nProgress: {:.2}%", progress));
            return Handled::Yes;
        }
        if let Some(error) = cmd.get(UPDATE_ERROR) {
            data.error = error.clone();
            return Handled::Yes;
        }
        if let Some(info) = cmd.get(UPDATE_INFO) {
            data.info.push_str(&format!("\n{}", info));
            return Handled::Yes;
        }
        if let Some(file_name) = cmd.get(UPDATE_FILE_NAME) {
            data.info.push_str(&format!("\nFile name: {}", file_name));
            return Handled::Yes;
        }
        if let Some(blob_name) = cmd.get(UPDATE_BLOB_NAME) {
            data.info.push_str(&format!("\nBlob name: {}", blob_name));
            return Handled::Yes;
        }
        Handled::No
    }
}

impl Data for Uploader {
    fn same(&self, other: &Self) -> bool {
        self.container == other.container
            && self.folder_path == other.folder_path
            && self.upload_folder == other.upload_folder
            && self.storage_account == other.storage_account
            && self.storage_account_key == other.storage_account_key
            && self.error == other.error
            && self.info == other.info
    }
}
