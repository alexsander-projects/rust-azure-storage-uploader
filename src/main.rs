use azure_storage::prelude::*;
use azure_storage_blobs::prelude::*;
use std::fs;
use std::io::Read;
use tokio::runtime::Runtime;
use druid::{AppLauncher, Widget, WindowDesc, widget::{TextBox, Button, Flex, Label}, Data, Lens, Env, EventCtx, Command, Target, WidgetExt};
use druid::commands::QUIT_APP;
use std::path::{Path, PathBuf};
use futures::future::try_join_all;

#[derive(Clone, Data, Lens)]
struct Uploader {
    container: String,
    folder_path: String,
    upload_folder: String,
}

fn ui_builder() -> impl Widget<Uploader> {
    let container_input = Flex::row()
        .with_child(Label::new("Container:"))
        .with_child(TextBox::new().lens(Uploader::container));

    let folder_path_input = Flex::row()
        .with_child(Label::new("Folder Path:"))
        .with_child(TextBox::new().lens(Uploader::folder_path));

    let upload_folder_input = Flex::row()
        .with_child(Label::new("Upload Folder:"))
        .with_child(TextBox::new().lens(Uploader::upload_folder));

    let upload_button = Button::new("Upload").on_click(move |_ctx, data: &mut Uploader, _env| {
        let container = data.container.clone();
        let folder_path = data.folder_path.clone();
        let upload_folder = data.upload_folder.clone();

        tokio::spawn(async move {
            let account = std::env::var("STORAGE_ACCOUNT").expect("Set env variable STORAGE_ACCOUNT first!");
            let access_key = std::env::var("STORAGE_ACCESS_KEY").expect("Set env variable STORAGE_ACCESS_KEY first!");

            let storage_credentials = StorageCredentials::access_key(account.clone(), access_key);
            let blob_service_client = BlobServiceClient::new(account, storage_credentials);

            let entries = fs::read_dir(Path::new(&folder_path)).expect("Directory not found");

            let mut upload_futures = vec![];

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

                    println!("Started uploading {} to {}", file_name, blob_name);
                }
            }

            // Allow all blobs to upload.
            try_join_all(upload_futures).await.expect("Failed to upload files");

            println!("All files uploaded successfully!");
        });
    });

    Flex::column()
        .with_child(container_input)
        .with_child(folder_path_input)
        .with_child(upload_folder_input)
        .with_child(upload_button)
}

fn main() {
    let main_window = WindowDesc::new(ui_builder())
        .title("Azure Storage Uploader");

    let initial_state = Uploader {
        container: String::new(),
        folder_path: String::new(),
        upload_folder: String::new(),
    };

    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        AppLauncher::with_window(main_window)
            .launch(initial_state)
            .expect("Failed to launch application");
    });
}