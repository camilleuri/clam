//! BAKE - the CLAM web server.

// Example API for testing purposes.

use poem::{listener::TcpListener, Route};
use poem_openapi::{param::Query, payload::PlainText, OpenApi, OpenApiService, Tags};
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;
use uuid::Uuid;

#[derive(Tags)]
enum Labels {
    Test
}

struct Api {
    tmp_dir: tempfile::TempDir,
}

#[OpenApi(tag = "Labels::Test")]
impl Api {
    /// Hello User example function.
    #[oai(path = "/hello", method = "get")]
    async fn hello(&self, name: Query<Option<String>>) -> PlainText<String> {
        match name.0 {
            Some(name) => PlainText(format!("hello, {}!", name)),
            None => PlainText("hello!".to_string()),
        }
    }

    /// Test file generation.
    #[oai(path = "/write", method = "get")]
    async fn write(&self, data: Query<Option<String>>) -> PlainText<String> {
        let id = Uuid::new_v4();
        let file_path = self.tmp_dir.path().join(format!("{}.txt", id.to_string()));
        let mut tmp_file = File::create(&file_path).expect("Failed to create file");
        writeln!(tmp_file, "{}", data.as_deref().unwrap_or("")).expect("Failed to write to file");
        PlainText(format!("Your data's UUID is {}. Do not lose it. Only share it with those you trust to access or delete your data.", id.to_string()))
    }
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let tmp_dir = tempdir()?;
    println!("Working from directory {:?}", tmp_dir.path());
    let api_service =
        OpenApiService::new(Api { tmp_dir }, "BAKE API", "1.0").server("http://localhost:80/api");
    let ui = api_service.swagger_ui();
    let app = Route::new().nest("/api", api_service).nest("/", ui);

    poem::Server::new(TcpListener::bind("0.0.0.0:80"))
        .run(app)
        .await
}