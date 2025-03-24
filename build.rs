#[cfg(not(any(feature = "use-minified")))]
use sha2::{Digest, Sha256};
#[cfg(not(any(feature = "use-minified")))]
use std::collections::HashMap;
#[cfg(not(any(feature = "use-minified")))]
use std::fs;
use std::io::Result;
#[cfg(not(any(feature = "use-minified")))]
use std::path::Path;
use std::path::PathBuf;
#[cfg(not(any(feature = "use-minified")))]
use std::process::Command;

// 支持的文件类型
#[cfg(not(any(feature = "use-minified")))]
const SUPPORTED_EXTENSIONS: [&str; 4] = ["html", "js", "css", "md"];

#[cfg(not(any(feature = "use-minified")))]
fn check_and_install_deps() -> Result<()> {
    let scripts_dir = Path::new("scripts");
    let node_modules = scripts_dir.join("node_modules");

    if !node_modules.exists() {
        println!("cargo:warning=Installing minifier dependencies...");
        let status = Command::new("npm")
            .current_dir(scripts_dir)
            .arg("install")
            .status()?;

        if !status.success() {
            panic!("Failed to install npm dependencies");
        }
        println!("cargo:warning=Dependencies installed successfully");
    }
    Ok(())
}

#[cfg(not(any(feature = "use-minified")))]
fn get_files_hash() -> Result<HashMap<PathBuf, String>> {
    let mut file_hashes = HashMap::new();
    let static_dir = Path::new("static");

    // 首先处理 README.md
    let readme_path = Path::new("README.md");
    if readme_path.exists() {
        let content = fs::read(readme_path)?;
        let mut hasher = Sha256::new();
        hasher.update(&content);
        let hash = format!("{:x}", hasher.finalize());
        file_hashes.insert(readme_path.to_path_buf(), hash);
    }

    if static_dir.exists() {
        for entry in fs::read_dir(static_dir)? {
            let entry = entry?;
            let path = entry.path();

            // 检查是否是支持的文件类型，且不是已经压缩的文件
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if SUPPORTED_EXTENSIONS.contains(&ext) && !path.to_string_lossy().contains(".min.")
                {
                    let content = fs::read(&path)?;
                    let mut hasher = Sha256::new();
                    hasher.update(&content);
                    let hash = format!("{:x}", hasher.finalize());
                    file_hashes.insert(path, hash);
                }
            }
        }
    }

    Ok(file_hashes)
}

#[cfg(not(any(feature = "use-minified")))]
fn load_saved_hashes() -> Result<HashMap<PathBuf, String>> {
    let hash_file = Path::new("scripts/.asset-hashes.json");
    if hash_file.exists() {
        let content = fs::read_to_string(hash_file)?;
        let hash_map: HashMap<String, String> = serde_json::from_str(&content)?;
        Ok(hash_map
            .into_iter()
            .map(|(k, v)| (PathBuf::from(k), v))
            .collect())
    } else {
        Ok(HashMap::new())
    }
}

#[cfg(not(any(feature = "use-minified")))]
fn save_hashes(hashes: &HashMap<PathBuf, String>) -> Result<()> {
    let hash_file = Path::new("scripts/.asset-hashes.json");
    let string_map: HashMap<String, String> = hashes
        .iter()
        .map(|(k, v)| (k.to_string_lossy().into_owned(), v.clone()))
        .collect();
    let content = serde_json::to_string_pretty(&string_map)?;
    fs::write(hash_file, content)?;
    Ok(())
}

#[cfg(not(any(feature = "use-minified")))]
fn minify_assets() -> Result<()> {
    // 获取现有文件的哈希
    let current_hashes = get_files_hash()?;

    if current_hashes.is_empty() {
        println!("cargo:warning=No files to minify");
        return Ok(());
    }

    // 加载保存的哈希值
    let saved_hashes = load_saved_hashes()?;

    // 找出需要更新的文件
    let files_to_update: Vec<_> = current_hashes
        .iter()
        .filter(|(path, current_hash)| {
            let is_readme = path.file_name().is_some_and(|f| f == "README.md");
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

            // 为 README.md 和其他文件使用不同的输出路径检查
            let min_path = if is_readme {
                PathBuf::from("static/readme.min.html")
            } else {
                path.with_file_name(format!(
                    "{}.min.{}",
                    path.file_stem().unwrap().to_string_lossy(),
                    ext
                ))
            };

            // 检查压缩/转换后的文件是否存在
            if !min_path.exists() {
                return true;
            }

            // 检查原始文件是否发生变化
            saved_hashes.get(*path) != Some(*current_hash)
        })
        .map(|(path, _)| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();

    if files_to_update.is_empty() {
        println!("cargo:warning=No files need to be updated");
        return Ok(());
    }

    println!("cargo:warning=Minifying {} files...", files_to_update.len());

    // 运行压缩脚本
    let status = Command::new("node")
        .arg("scripts/minify.js")
        .args(&files_to_update)
        .status()?;

    if !status.success() {
        panic!("Asset minification failed");
    }

    // 保存新的哈希值
    save_hashes(&current_hashes)?;

    Ok(())
}

fn main() -> Result<()> {
    // Proto 文件处理
    // println!("cargo:rerun-if-changed=src/chat/aiserver/v1/lite.proto");
    println!("cargo:rerun-if-changed=src/chat/config/key.proto");
    // 获取环境变量 PROTOC
    let protoc_path = match std::env::var_os("PROTOC") {
        Some(path) => PathBuf::from(path),
        None => {
            println!("cargo:warning=PROTOC environment variable not set, using default protoc.");
            // 如果 PROTOC 未设置，则返回一个空的 PathBuf，prost-build 会尝试使用默认的 protoc
            PathBuf::new()
        }
    };
    let mut config = prost_build::Config::new();
    // 如果 protoc_path 不为空，则配置使用指定的 protoc
    if !protoc_path.as_os_str().is_empty() {
        config.protoc_executable(protoc_path);
    }
    // config.type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]");
    // config.enum_attribute(".aiserver.v1", "#[allow(clippy::enum_variant_names)]");
    // config
    //     .compile_protos(
    //         &["src/chat/aiserver/v1/lite.proto"],
    //         &["src/chat/aiserver/v1/"],
    //     )
    //     .unwrap();
    config
        .compile_protos(&["src/chat/config/key.proto"], &["src/chat/config/"])
        .unwrap();

    // 静态资源文件处理
    println!("cargo:rerun-if-changed=scripts/minify.js");
    println!("cargo:rerun-if-changed=scripts/package.json");
    println!("cargo:rerun-if-changed=static/api.html");
    println!("cargo:rerun-if-changed=static/build_key.html");
    println!("cargo:rerun-if-changed=static/config.html");
    println!("cargo:rerun-if-changed=static/logs.html");
    println!("cargo:rerun-if-changed=static/shared-styles.css");
    println!("cargo:rerun-if-changed=static/shared.js");
    println!("cargo:rerun-if-changed=static/tokens.html");
    println!("cargo:rerun-if-changed=README.md");

    #[cfg(not(any(feature = "use-minified")))]
    {
        // 检查并安装依赖
        check_and_install_deps()?;

        // 运行资源压缩
        minify_assets()?;
    }

    Ok(())
}
