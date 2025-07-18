use std::path::Path;
use std::{env, fs};

fn main() {
    copy_file("keypair.bin");
    copy_file("dex_data.json");
    // // 源文件：同级目录下的 keypair.bin
    // let src = Path::new("keypair.bin");
    // if !src.exists() {
    //     panic!("keypair.bin 文件不存在！");
    // }
    //
    // // 获取构建输出目录（根据 Cargo 自动确定）
    // let out_dir = env::var("OUT_DIR").expect("无法获取 OUT_DIR");
    // let out_path = Path::new(&out_dir)
    //     .join("../../../..")
    //     .canonicalize()
    //     .unwrap();
    //
    // // 确保输出路径是 target/release 或 target/debug
    // let is_release = env::var("PROFILE").unwrap_or_default() == "release";
    // let target_dir = if is_release {
    //     out_path.join("release")
    // } else {
    //     out_path.join("debug")
    // };
    //
    // // 确保目标目录存在
    // fs::create_dir_all(&target_dir).expect("无法创建目标目录");
    //
    // // 将文件复制到构建输出目录
    // let dest = target_dir.join("keypair.bin");
    // fs::copy(src, &dest).expect("无法复制文件");
    //
    // println!("文件已复制到：{}", dest.display());

}

fn copy_file(path: &str) {
    // 源文件：同级目录下的 keypair.bin
    let src = Path::new(path);
    if !src.exists() {
        panic!("{:?} 文件不存在！", path);
    }
    // 获取构建输出目录（根据 Cargo 自动确定）
    let out_dir = env::var("OUT_DIR").expect("无法获取 OUT_DIR");
    let out_path = Path::new(&out_dir)
        .join("../../../..")
        .canonicalize()
        .unwrap();

    // 确保输出路径是 target/release 或 target/debug
    let is_release = env::var("PROFILE").unwrap_or_default() == "release";
    let target_dir = if is_release {
        out_path.join("release")
    } else {
        out_path.join("debug")
    };

    // 确保目标目录存在
    fs::create_dir_all(&target_dir).expect("无法创建目标目录");

    // 将文件复制到构建输出目录
    let dest = target_dir.join(path);
    fs::copy(src, &dest).expect("无法复制文件");
    println!("文件已复制到：{}", dest.display());
    // println!("cargo:rerun-if-changed=keypair.bin");
}
