use azure_storage::prelude::*;
use azure_storage_blobs::prelude::*;
use std::fs;
use std::io::Read;
use druid::{
    AppLauncher, Widget, WindowDesc,
    widget::{TextBox, Button, Flex, Label, ProgressBar as DruidProgressBar},
    Data, Lens, Env, Command, Target, WidgetExt, AppDelegate, DelegateCtx, Handled, Selector,
};
use std::path::Path;
use futures::future::try_join_all;
use indicatif::{ProgressBar, ProgressStyle};

#[derive(Clone, Data, Lens)]
struct Uploader {
    container: String,
    folder_path: String,
    upload_folder: String,
    storage_account: String,
    storage_account_key: String,
    info: String,
    progress: f64,
    error: String,
}

const UPDATE_PROGRESS: Selector<f64> = Selector::new("uploader.update-progress");
const UPDATE_ERROR: Selector<String> = Selector::new("uploader.update-error");
const UPDATE_INFO: Selector<String> = Selector::new("uploader.update-info");

fn ui_builder() -> impl Widget<Uploader> {
    let info_label = Label::new(|data: &Uploader, _env: &Env| {
        format!("{}", data.info)
    });

    let progress_bar = DruidProgressBar::new().lens(Uploader::progress);

    let error_label = Label::new(|data: &Uploader, _env: &Env| {
        format!("{}", data.error)
    }).with_text_color(druid::piet::Color::rgb8(255, 0, 0));

    let container_input = Flex::row()
        .with_child(Label::new("Container:"))
        .with_child(TextBox::new().lens(Uploader::container));

    let folder_path_input = Flex::row()
        .with_child(Label::new("Folder Path:"))
        .with_child(TextBox::new().lens(Uploader::folder_path));

    let upload_folder_input = Flex::row()
        .with_child(Label::new("Upload Folder:"))
        .with_child(TextBox::new().lens(Uploader::upload_folder));

    let storage_account_input = Flex::row()
        .with_child(Label::new("Storage Account:"))
        .with_child(TextBox::new().lens(Uploader::storage_account));

    let storage_account_key_input = Flex::row()
        .with_child(Label::new("Storage Account Key:"))
        .with_child(TextBox::new().lens(Uploader::storage_account_key));

    let upload_button = Button::new("Upload").on_click(move |ctx, data: &mut Uploader, _env| {
        let container = data.container.clone();
        let folder_path = data.folder_path.clone();
        let upload_folder = data.upload_folder.clone();
        let account = data.storage_account.clone();
        let access_key = data.storage_account_key.clone();
        let ext_event_sink = ctx.get_external_handle();

        data.info = format!("Uploading files from {} to {}/{} in container {}...", folder_path, account, upload_folder, container);
        data.error.clear(); // Clear previous errors
        data.progress = 0.0;

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
            rt.block_on(async move {
                let storage_credentials = StorageCredentials::access_key(account.clone(), access_key.clone());
                let blob_service_client = BlobServiceClient::new(account.clone(), storage_credentials);

                let entries: Vec<_> = match fs::read_dir(Path::new(&folder_path)) {
                    Ok(entries) => entries.collect(),
                    Err(err) => {
                        ext_event_sink.submit_command(UPDATE_ERROR, format!("Directory read error: {}", err), Target::Auto).unwrap();
                        return;
                    }
                };

                let total_files = entries.len() as u64;
                let mut upload_futures = vec![];

                let pb = ProgressBar::new(total_files);
                pb.set_style(ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                    .expect("Failed to set progress bar template")
                    .progress_chars("#>-"));

                for (idx, entry) in entries.into_iter().enumerate() {
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(err) => {
                            ext_event_sink.submit_command(UPDATE_ERROR, format!("Failed to read entry: {}", err), Target::Auto).unwrap();
                            continue;
                        }
                    };

                    let path = entry.path();
                    if path.is_file() {
                        let file_name = match path.file_name() {
                            Some(name) => match name.to_str() {
                                Some(name_str) => name_str.to_string(),
                                None => {
                                    ext_event_sink.submit_command(UPDATE_ERROR, "Failed to convert file name to string".to_string(), Target::Auto).unwrap();
                                    continue;
                                }
                            },
                            None => {
                                ext_event_sink.submit_command(UPDATE_ERROR, "Failed to get file name".to_string(), Target::Auto).unwrap();
                                continue;
                            }
                        };

                        let blob_name = format!("{}/{}", upload_folder, file_name);

                        let mut file = match fs::File::open(&path) {
                            Ok(file) => file,
                            Err(err) => {
                                ext_event_sink.submit_command(UPDATE_ERROR, format!("Failed to open file {}: {}", file_name, err), Target::Auto).unwrap();
                                continue;
                            }
                        };

                        let mut data = Vec::new();
                        if let Err(err) = file.read_to_end(&mut data) {
                            ext_event_sink.submit_command(UPDATE_ERROR, format!("Failed to read file {}: {}", file_name, err), Target::Auto).unwrap();
                            continue;
                        }

                        let blob_client = blob_service_client
                            .container_client(&container)
                            .blob_client(&blob_name);

                        let task = blob_client.put_block_blob(data).into_future();
                        upload_futures.push(task);

                        pb.inc(1);
                        ext_event_sink.submit_command(UPDATE_PROGRESS, ((idx as f64 + 1.0) / total_files as f64) * 100.0, Target::Auto).unwrap();
                    }
                }

                // Allow all blobs to upload.
                match try_join_all(upload_futures).await {
                    Ok(_) => {
                        pb.finish_with_message("All files uploaded successfully!");
                        ext_event_sink.submit_command(UPDATE_PROGRESS, 100.0, Target::Auto).unwrap();
                        ext_event_sink.submit_command(UPDATE_INFO, "All files uploaded successfully!".to_string(), Target::Auto).unwrap();
                    }
                    Err(err) => {
                        ext_event_sink.submit_command(UPDATE_ERROR, format!("Failed to upload files: {:?}", err), Target::Auto).unwrap();
                    }
                }
            });
        });
    });

    Flex::column()
        .with_child(container_input)
        .with_child(folder_path_input)
        .with_child(upload_folder_input)
        .with_child(storage_account_input)
        .with_child(storage_account_key_input)
        .with_child(upload_button)
        .with_child(info_label)
        .with_child(progress_bar)
        .with_child(error_label)
}

fn main() {
    let main_window = WindowDesc::new(ui_builder())
        .title("Azure Storage Uploader");

    let initial_state = Uploader {
        container: String::new(),
        folder_path: String::new(),
        upload_folder: String::new(),
        storage_account: String::new(),
        storage_account_key: String::new(),
        info: String::new(),
        progress: 0.0,
        error: String::new(),
    };

    AppLauncher::with_window(main_window)
        .delegate(Delegate)
        .launch(initial_state)
        .expect("Failed to launch application");
}

struct Delegate;

impl AppDelegate<Uploader> for Delegate {
    fn command(&mut self, _: &mut DelegateCtx, _: Target, cmd: &Command, data: &mut Uploader, _: &Env) -> Handled {
        if let Some(progress) = cmd.get(UPDATE_PROGRESS) {
            data.progress = progress.clone();
            return Handled::Yes;
        }
        if let Some(error) = cmd.get(UPDATE_ERROR) {
            data.error = error.clone();
            return Handled::Yes;
        }
        if let Some(info) = cmd.get(UPDATE_INFO) {
            data.info = info.clone();
            return Handled::Yes;
        }
        Handled::No
    }
}
