use std::path::Path;

pub fn toml_path_value(path: &Path) -> String {
    toml::Value::String(path.display().to_string()).to_string()
}
