use router::dex::json_state::state::ClmmJsonInfo;
use router::file_db::FILE_DB_DIR;
use std::env;
use std::fs::File;
use tracing::error;

#[test]
fn test() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    println!("manifest_dir: {}", manifest_dir);
    let path = format!("{}/src/data/dex_data.json", manifest_dir);
    let pool_infos: Vec<ClmmJsonInfo> =
        match File::open(path) {
            Ok(file) => serde_json::from_reader(file).expect("Could not parse JSON"),
            Err(e) => {
                error!("{}", e);
                vec![]
            }
        };
    println!("pool_infos: {:?}", pool_infos);
}
