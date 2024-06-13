use azure_storage::prelude::*;
use azure_storage_blobs::prelude::*;
use futures::future::try_join_all;
use std::fs;
use std::io::Read;
use std::path::Path;

#[tokio::main]
async fn main() -> azure_core::Result<()> {
    // Retrieve the account name and access key from environment variables.
    let account = std::env::var("STORAGE_ACCOUNT").expect("Set env variable STORAGE_ACCOUNT first!");
    let access_key = std::env::var("STORAGE_ACCESS_KEY").expect("Set env variable STORAGE_ACCESS_KEY first!");

    let container = std::env::args()
        .nth(1)
        .expect("please specify container name as command line parameter");
    let folder_path = std::env::args()
        .nth(2)
        .expect("please specify folder path as command line parameter");
    let upload_folder = std::env::args()
        .nth(3)
        .expect("please specify upload folder as command line parameter");

    let storage_credentials = StorageCredentials::access_key(account.clone(), access_key);
    let blob_service_client = BlobServiceClient::new(account, storage_credentials);

    // Read the directory
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
    try_join_all(upload_futures).await?;

    println!("All files uploaded successfully!!");

    Ok(())
}