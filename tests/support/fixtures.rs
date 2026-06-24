use std::{fs, path::Path};

use serde_json::Value;

pub fn read_fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("读取测试夹具失败 {}: {error}", path.display()));

    serde_json::from_str(&content)
        .unwrap_or_else(|error| panic!("解析测试夹具失败 {}: {error}", path.display()))
}

pub fn read_text_fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("读取测试夹具失败 {}: {error}", path.display()))
}

pub fn read_binary_fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    fs::read(&path).unwrap_or_else(|error| panic!("读取测试夹具失败 {}: {error}", path.display()))
}
