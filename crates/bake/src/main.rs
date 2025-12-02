//! BAKE - the CLAM web server.

// TODO:
// Test rNN
// Use a metadata file per upload instead of a single monolithic file
// Make /download return a txt file instead of an octet buffer
// Specify features of abd-clam used

// Allow rNN and kNN to take different metrics
// Allow rNN and kNN to specify criteria/query
// Allow rNN and kNN to take datasets of varying dimensionality/type
// Allow rNN and kNN to fail gracefully and write failure state to UUID file
// Allow rNN and kNN to return UUID early and continue working

use abd_clam::{Ball, Cluster, FlatVec, cakes::{self, SearchAlgorithm}, cluster::Partition, dataset::AssociatesMetadataMut};
use poem::{listener::TcpListener, Route};
use poem_openapi::{param::Query, payload::{PlainText, Binary}, OpenApi, OpenApiService, Tags};
use rand::prelude::*;
use serde_json::{self, Error};
use std::{fs, io::{Read, Seek}};
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
        f.lock().expect("Failed to lock file");
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
    f.lock().expect("Failed to lock file");
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
    f.set_len(0).expect("Failed to reset file (zero length)");
    f.rewind().expect("Failed to reset file (zero head)");
    f.write_all(json_value.to_string().as_bytes()).expect("Failed to write to file");
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
        write_index(uuid.to_string(), "Deleted".to_string(), &self.tmp_dir);
        match fs::remove_file(file_path) {
            Ok(_) => PlainText(format!("UUID {} successfully deleted.", uuid.to_string())),
            Err(e) => PlainText(format!("UUID {} deletion failed with error {}.", uuid.to_string(), e.to_string())),
        }
    }

    /// Download a dataset or query result from the server by UUID. For now, download must be manually renamed to the proper file extention.
    #[oai(path = "/download", method = "get", tag = "Labels::FileManagement")]
    async fn download(&self, uuid: Query<String>) -> Binary<Vec<u8>> {
        let file_path = self.tmp_dir.path().join(format!("{}.txt", uuid.to_string()));
        let mut f = fs::File::open(file_path).unwrap();
        f.lock().expect("Failed to lock file");
        let mut buffer = Vec::new();
        f.read_to_end(&mut buffer).expect("Failed to read file");
        poem_openapi::payload::Binary(buffer)
    }

    // Information functions.

    /// Get basic information about an item from its UUID,
    /// whether it is a dataset, in progress query, or completed query.
    #[oai(path = "/about", method = "get", tag = "Labels::Information")]
    async fn about(&self, uuid: Query<String>) -> PlainText<String> {
        PlainText(read_index(uuid.to_string(), &self.tmp_dir).to_string())
    }

    // Query functions.

    /// Begin a rNN search on a dataset. Returns a query UUID.
    #[oai(path = "/rnn", method = "get", tag = "Labels::Queries")]
    async fn rnn(&self, 
        rows_uuid: Query<String>, 
        labels_uuid: Query<String>,
        metric: Query<String>,
        dimensionality: Query<usize>,
        radius: Query<f32>,
        seed: Query<Option<usize>>
    ) -> PlainText<String> {

        let rows_path = self.tmp_dir.path().join(format!("{}.txt", rows_uuid.to_string()));
        let rows_file_contents= fs::read_to_string(rows_path).expect("Failed to read rows as string");
        let rows: Vec<Vec<f32>> = serde_json::from_str(&rows_file_contents).expect("Failed to read string of rows as vector");

        let labels_path = self.tmp_dir.path().join(format!("{}.txt", labels_uuid.to_string()));
        let labels_file_contents= fs::read_to_string(labels_path).expect("Failed to read labels as string");
        let labels: Vec<bool> = serde_json::from_str(&labels_file_contents).expect("Failed to read string of labels as vector");
        println!("{:?}", rows);
        println!("{:?}", labels);

        let data = FlatVec::new(rows).unwrap().with_metadata(&labels).unwrap();

        let clam_metric = match metric.as_str() {
            "euclidean" => Ok(abd_clam::metric::Euclidean),
            // todo!("Camille")
            &_ => Err("Incorrect metric specified"),
        }.expect("Incorrect metric specified");

        let criteria = |c: &Ball<_>| c.cardinality() > 1; // todo!("Camille")

        let query = vec![0_f32; *dimensionality]; // todo!("Camille")

        let root_seed: usize = match *seed {
            Some(i) => i,
            None => 42
        };

        let root = Ball::new_tree(&data, &clam_metric, &criteria, Some((root_seed).try_into().unwrap()));

        let alg = cakes::RnnClustered(*radius);
        let rnn_results: Vec<(usize, f32)> = alg.search(&data, &clam_metric, &root, &query);
        println!("{:?}", rnn_results);
        let results_json = serde_json::to_string(&rnn_results).expect("Failed to convert results to json");

        let id = Uuid::new_v4();
        let file_path = self.tmp_dir.path().join(format!("{}.txt", id.to_string()));
        fs::write(file_path, results_json).expect("Failed to write results to file");
        write_index(id.to_string(), "Query result".to_string(), &self.tmp_dir);

        PlainText(format!("Query finished with UUID {}.", id.to_string()))
    }

    // Test functions for development purposes, remove before shipping

    /// Return the full index file
    #[oai(path = "/fullindex", method = "get", tag = "Labels::DevFunc")]
    async fn fullindex(&self) -> PlainText<String> {
        let file_path = self.tmp_dir.path().join("index.json");
        let contents = fs::read_to_string(file_path);
        poem_openapi::payload::PlainText(contents.unwrap())
    }

    /// Generate random dataset for testing using symagen
    #[oai(path = "/symagen", method = "get", tag = "Labels::DevFunc")]
    async fn symagen(&self) -> PlainText<String> {
        let seed = 42;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let (cardinality, dimensionality) = (1_000, 10);
        let (min_val, max_val) = (-1.0, 1.0);
        let rows: Vec<Vec<f32>> = 
            symagen::random_data::random_tabular(cardinality, dimensionality, min_val, max_val, &mut rng);
        let labels: Vec<bool> = rows.iter().map(|v| v[0] > 0.0).collect();
        
        let rows_json = serde_json::to_string(&rows).expect("Failed to convert json to string");
        let rows_id = Uuid::new_v4();
        let rows_file_path = self.tmp_dir.path().join(format!("{}.txt", rows_id.to_string()));
        let mut rows_tmp_file = fs::File::create(&rows_file_path).expect("Failed to create file");
        rows_tmp_file.write_all(rows_json.as_ref()).expect("Failed to write data");
        write_index(rows_id.to_string(), "Dataset (symagen rows)".to_string(), &self.tmp_dir);

        let labels_json = serde_json::to_string(&labels).expect("Failed to convert json to string");
        let labels_id = Uuid::new_v4();
        let labels_file_path = self.tmp_dir.path().join(format!("{}.txt", labels_id.to_string()));
        let mut labels_tmp_file = fs::File::create(&labels_file_path).expect("Failed to create file");
        labels_tmp_file.write_all(labels_json.as_ref()).expect("Failed to write data");
        write_index(labels_id.to_string(), "Dataset (symagen labels)".to_string(), &self.tmp_dir);

        PlainText(format!("Rows UUID: {}, Labels UUID: {}", rows_id.to_string(), labels_id.to_string()))
    }
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let tmp_dir = tempdir()?;
    println!("Working from directory {:?}\nQuick link: http://localhost", tmp_dir.path());
    let api_service =
        OpenApiService::new(Api { tmp_dir }, "URI-ABD BAKE API", "0.1.0").server("http://localhost:80/api");
    let ui = api_service.swagger_ui();
    let app = Route::new().nest("/api", api_service).nest("/", ui);

    poem::Server::new(TcpListener::bind("0.0.0.0:80"))
        .run(app)
        .await
}