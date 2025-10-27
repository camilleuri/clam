//! BAKE - the CLAM web server.

use poem::{listener::TcpListener, Route};
use poem_openapi::{param::Query, payload::{PlainText, Binary}, OpenApi, OpenApiService, Tags};
use serde_json::{self, Error};
use std::{fs, io::Seek};
use std::io::{Write, BufReader};
use std::path::Path;
use tempfile::tempdir;
use uuid::Uuid;

#[derive(Tags)]
enum Labels {
    FileManagement,
    Information,
    Queries,
    DevFunc
}

struct Api {
    tmp_dir: tempfile::TempDir,
}

// Helper functions.

// read_index and write_index operate on the index,
// which is a JSON file located at index.json,
// and contains metadata on all files in the temp directory.

fn read_index(uuid: String, tmp_dir: &tempfile::TempDir) -> String {
    let file_path = tmp_dir.path().join("index.json");
    if Path::new(&file_path).exists() {
        let f = fs::File::open(file_path).unwrap();
        let _ = f.lock();
        let reader = BufReader::new(f);
        let json_value: serde_json::Value = serde_json::from_reader(reader).unwrap();

        let uuid_val = json_value[uuid].clone();
            if uuid_val != serde_json::Value::Null {
                uuid_val.to_string()
            } else {
                "None".to_string()
            }
    } else {
        "None".to_string()
    }
}

fn write_index(uuid: String, to_write: String, tmp_dir: &tempfile::TempDir) {
    let file_path = tmp_dir.path().join("index.json");
    let mut f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(file_path)
            .unwrap();
    let _ = f.lock();
    let reader = BufReader::new(&f);
    let json_attempt: Result<serde_json::Value, Error> = serde_json::from_reader(reader);
    let mut json_value: serde_json::Value;
    match json_attempt {
        Ok(_) => {
            json_value = json_attempt.unwrap().clone();
            json_value[uuid] = serde_json::json!(to_write)
        }
        Err(_) => json_value = serde_json::json!({uuid: to_write})
    }
    let _ = f.set_len(0);
    let _ = f.rewind();
    let _ = f.write_all(json_value.to_string().as_bytes());
}

#[OpenApi]
impl Api {

    // File management functions.

    /// Upload a dataset to the server.
    #[oai(path = "/upload", method = "post", tag = "Labels::FileManagement")]
    async fn upload(&self, data: Binary<Vec<u8>>) -> PlainText<String> {
        let id = Uuid::new_v4();
        let file_path = self.tmp_dir.path().join(format!("{}.txt", id.to_string()));
        let mut tmp_file = fs::File::create(&file_path).expect("Failed to create file");
        tmp_file.write_all(data.0.as_ref()).expect("Failed to write data");
        write_index(id.to_string(), "Dataset".to_string(), &self.tmp_dir);

        PlainText(format!("Your data's UUID is {}. Do not lose it. Only share it with those you trust to access or delete your data.", id.to_string()))
    }

    /// Delete a dataset or query result by UUID.
    #[oai(path = "/delete", method = "delete", tag = "Labels::FileManagement")]
    async fn delete(&self, uuid: Query<String>) -> PlainText<String> {
        let file_path = self.tmp_dir.path().join(format!("{}.txt", uuid.to_string()));
        todo!("Camille - Use write_index to set the removed file to Deleted");
        match fs::remove_file(file_path) {
            Ok(_) => PlainText(format!("UUID {} successfully deleted.", uuid.to_string())),
            Err(e) => PlainText(format!("UUID {} deletion failed with error {}.", uuid.to_string(), e.to_string())),
        }
    }

    /// Download a dataset or query result from the server by UUID.
    #[oai(path = "/download", method = "get", tag = "Labels::FileManagement")]
    async fn download(&self, _uuid: Query<String>) -> PlainText<String> {
        todo!("Camille");
    }

    // Information functions.

    /// Get basic information about an item from its UUID,
    /// whether it is a dataset, in progress query, or completed query.
    #[oai(path = "/about", method = "get", tag = "Labels::Information")]
    async fn about(&self, uuid: Query<String>) -> PlainText<String> {
        PlainText(read_index(uuid.to_string(), &self.tmp_dir).to_string())
    }

    // Query functions.

    /// Execute some query, such as search, on a dataset.
    #[oai(path = "/query", method = "get", tag = "Labels::Queries")]
    async fn query(&self, _dataset_uuid: Query<String>) -> PlainText<String> {
        todo!("Camille");
    }

    // Test functions for development purposes, remove before shipping

    /// Return the full index file
    #[oai(path = "/fullindex", method = "get", tag = "Labels::DevFunc")]
    async fn fullindex(&self) -> PlainText<String> {
        let file_path = self.tmp_dir.path().join("index.txt");
        let contents = fs::read_to_string(file_path);
        poem_openapi::payload::PlainText(contents.unwrap())
    }
    
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let tmp_dir = tempdir()?;
    println!("Working from directory {:?}", tmp_dir.path());
    let api_service =
        OpenApiService::new(Api { tmp_dir }, "BAKE API", "0.1.0").server("http://localhost:80/api");
    let ui = api_service.swagger_ui();
    let app = Route::new().nest("/api", api_service).nest("/", ui);

    poem::Server::new(TcpListener::bind("0.0.0.0:80"))
        .run(app)
        .await
}