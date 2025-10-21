//! BAKE - the CLAM web server.

// Example API for testing purposes.

use poem::{listener::TcpListener, Route};
use poem_openapi::{param::Query, payload::{PlainText, Binary}, OpenApi, OpenApiService, Tags};
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;
use uuid::Uuid;

#[derive(Tags)]
enum Labels {
    FileManagement,
    Information,
    Queries
}

struct Api {
    tmp_dir: tempfile::TempDir,
}

#[OpenApi]
impl Api {

    /// Upload a dataset to the server.
    #[oai(path = "/upload", method = "post", tag = "Labels::FileManagement")]
    async fn upload(&self, data: Binary<Vec<u8>>) -> PlainText<String> {
        let id = Uuid::new_v4();
        let file_path = self.tmp_dir.path().join(format!("{}.txt", id.to_string()));
        let mut tmp_file = File::create(&file_path).expect("Failed to create file");
        tmp_file.write_all(data.0.as_ref()).expect("Failed to write data");

        PlainText(format!("Your data's UUID is {}. Do not lose it. Only share it with those you trust to access or delete your data.", id.to_string()))
    }

    /// Delete a dataset or query result by UUID.
    #[oai(path = "/delete", method = "delete", tag = "Labels::FileManagement")]
    async fn delete(&self, uuid: Query<String>) -> PlainText<String> {
        todo!("Camille");
        PlainText("".to_string())
    }

    /// Download a dataset or query result from the server by UUID.
    #[oai(path = "/download", method = "get", tag = "Labels::FileManagement")]
    async fn download(&self, uuid: Query<String>) -> PlainText<String> {
        todo!("Camille");
        PlainText("".to_string())
    }

    /// Get basic information about an item from its UUID,
    /// whether it is a dataset, in progress query, or completed query.
    #[oai(path = "/about", method = "get", tag = "Labels::Information")]
    async fn about(&self, uuid: Query<String>) -> PlainText<String> {
        todo!("Camille");
        PlainText("".to_string())
    }

    /// Execute some query, such as search, on a dataset.
    #[oai(path = "/query", method = "get", tag = "Labels::Queries")]
    async fn query(&self, dataset_uuid: Query<String>) -> PlainText<String> {
        todo!("Camille");
        PlainText("".to_string())
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