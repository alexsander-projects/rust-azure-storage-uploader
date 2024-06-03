# Rust-azure-storage-uploader

## Blazingly fast azure-storage uploader in Rust

- The app is able to upload files of any size. It uses the Azure Storage SDK for Rust to upload the files to the Azure
  Storage account.
- The app is able to upload files in parallel.

### How to run

- First, set the environment variables `AZURE_STORAGE_ACCOUNT` and `AZURE_STORAGE_ACCESS_KEY` with the credentials of your
  Azure Storage account.
- Then, run the app specifying the container name, the folder path, and the upload folder name.

```bash
cargo run --release -- <container> <folder_path> <upload_folder>
```

### Parameters

- container: The name of the container where the files will be uploaded.
- folder_path: The path to the folder that contains the files that will be uploaded.
- upload_folder: The name of the folder that will be created in the container to store the files.

