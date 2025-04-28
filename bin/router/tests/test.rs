use std::env;
use router::interface::Pool;
use std::fs::File;
use router::defi::raydium_amm::state::PoolInfo;

#[test]
fn test() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    println!("manifest_dir: {}", manifest_dir);
    let path = format!("{}/src/data/raydium_amm.json", manifest_dir);
    match File::open(path) {
        Ok(file) => {
            let result:Vec<PoolInfo> = serde_json::from_reader(file).expect("Could not parse JSON");
            println!("{:?}", result);
        }
        Err(e) => eprintln!("Failed to open file: {}", e),
    }
}
