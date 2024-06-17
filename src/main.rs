use azure_storage::prelude::*;
use azure_storage_blobs::prelude::*;
use std::fs;
use std::io::Read;
use druid::{
    AppLauncher, Widget, WindowDesc,
    widget::{TextBox, Button, Flex, Label},
    Data, Lens, Env, Command, Target, WidgetExt, ExtEventSink, AppDelegate, DelegateCtx, Handled, Selector,
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
    progress: String,
}

const UPDATE_PROGRESS: Selector<String> = Selector::new("uploader.update-progress");

fn ui_builder() -> impl Widget<Uploader> {
    let info_label = Label::new(|data: &Uploader, _env: &Env| {
        format!("{}", data.info)
    });

    let progress_label = Label::new(|data: &Uploader, _env: &Env| {
        format!("{}", data.progress)
    });

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

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
            rt.block_on(async move {
                let storage_credentials = StorageCredentials::access_key(account.clone(), access_key);
                let blob_service_client = BlobServiceClient::new(account, storage_credentials);

                let entries: Vec<_> = fs::read_dir(Path::new(&folder_path)).expect("Directory not found").collect();
                let mut upload_futures = vec![];

                let pb = ProgressBar::new(entries.len() as u64);
                pb.set_style(ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                    .expect("Failed to set progress bar template")
                    .progress_chars("#>-"));

                for entry in entries {
                    let entry = entry.expect("Failed to read entry");
                    let path = entry.path();
                    if path.is_file() {
                        let file_name = path.file_name().expect("Failed to get file name").to_str().expect("Failed to convert to str");
                        let blob_name = format!("{}/{}", upload_folder, file_name);

                        let mut file = fs::File::open(&path).expect("Failed to open file");
                        let mut data = Vec::new();
                        file.read_to_end(&mut data).expect("Failed to read file");

                        let blob_client = blob_service_client
                            .container_client(&container)
                            .blob_client(&blob_name);

                        let task = blob_client.put_block_blob(data).into_future();
                        upload_futures.push(task);

                        pb.inc(1);

                        ext_event_sink.submit_command(UPDATE_PROGRESS, format!("Started uploading {} to {}", file_name, blob_name), Target::Auto).unwrap();
                    }
                }

                // Allow all blobs to upload.
                try_join_all(upload_futures).await.expect("Failed to upload files");

                pb.finish_with_message("All files uploaded successfully!");
                ext_event_sink.submit_command(UPDATE_PROGRESS, "All files uploaded successfully!".to_string(), Target::Auto).unwrap();
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
        .with_child(progress_label)
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
        progress: String::new(),
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
        Handled::No
    }
}
