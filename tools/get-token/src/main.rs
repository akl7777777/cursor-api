use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rusqlite::Connection;
use std::env;
use std::path::PathBuf;
use std::process::Command;

fn get_machine_id() -> String {
    if cfg!(windows) {
        let output = Command::new("REG")
            .args(&[
                "QUERY",
                "HKEY_LOCAL_MACHINE\\SOFTWARE\\Microsoft\\Cryptography",
                "/v",
                "MachineGuid",
            ])
            .output()
            .expect("无法执行 REG 命令");
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .find(|line| line.contains("MachineGuid"))
            .and_then(|line| line.split_whitespace().last())
            .unwrap_or("unknown")
            .to_string()
    } else if cfg!(target_os = "macos") {
        let output = Command::new("ioreg")
            .args(&["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
            .expect("无法执行 ioreg 命令");
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .find(|line| line.contains("IOPlatformUUID"))
            .and_then(|line| line.split("\"").nth(3))
            .unwrap_or("unknown")
            .to_string()
    } else if cfg!(target_os = "linux") {
        let output = Command::new("sh")
            .arg("-c")
            .arg("( cat /var/lib/dbus/machine-id /etc/machine-id 2> /dev/null || hostname ) | head -n 1 || :")
            .output()
            .expect("无法获取 machine-id");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else if cfg!(target_os = "freebsd") {
        let output = Command::new("sh")
            .arg("-c")
            .arg("kenv -q smbios.system.uuid || sysctl -n kern.hostuuid")
            .output()
            .expect("无法获取 UUID");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else {
        "unknown".to_string()
    }
}

fn obfuscate_bytes(bytes: &mut [u8]) {
    let mut prev: u8 = 165;
    for (idx, byte) in bytes.iter_mut().enumerate() {
        let old_value = *byte;
        *byte = (old_value ^ prev).wrapping_add((idx % 256) as u8);
        prev = *byte;
    }
}

fn generate_timestamp_header() -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / 1_000;

    let mut timestamp_bytes = vec![
        ((timestamp >> 8) & 0xFF) as u8,
        (timestamp & 0xFF) as u8,
        ((timestamp >> 24) & 0xFF) as u8,
        ((timestamp >> 16) & 0xFF) as u8,
        ((timestamp >> 8) & 0xFF) as u8,
        (timestamp & 0xFF) as u8,
    ];

    obfuscate_bytes(&mut timestamp_bytes);
    BASE64.encode(&timestamp_bytes)
}

fn main() {
    let db_path = if cfg!(windows) {
        let app_data = env::var("APPDATA").unwrap_or_else(|_| {
            let profile = env::var("USERPROFILE").expect("未找到 USERPROFILE 环境变量");
            PathBuf::from(profile)
                .join("AppData")
                .join("Roaming")
                .to_string_lossy()
                .to_string()
        });
        PathBuf::from(app_data).join(r"Cursor\User\globalStorage\state.vscdb")
    } else if cfg!(target_os = "macos") {
        let home = env::var("HOME").expect("未找到 HOME 环境变量");
        PathBuf::from(home)
            .join("Library/Application Support/Cursor/User/globalStorage/state.vscdb")
    } else if cfg!(target_os = "linux") {
        let config_home = env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            let home = env::var("HOME").expect("未找到 HOME 环境变量");
            format!("{}/.config", home)
        });
        PathBuf::from(config_home).join("Cursor/User/globalStorage/state.vscdb")
    } else {
        panic!("不支持的操作系统平台")
    };

    match Connection::open(&db_path) {
        Ok(conn) => {
            let token = conn.query_row(
                "SELECT value FROM ItemTable WHERE key = 'cursorAuth/accessToken'",
                [],
                |row| row.get::<_, String>(0),
            );

            let storage_path = db_path.parent().unwrap().join("storage.json");
            let storage_content = std::fs::read_to_string(storage_path).unwrap_or_default();
            let storage_json: serde_json::Value =
                serde_json::from_str(&storage_content).unwrap_or_default();

            match token {
                Ok(token) => {
                    println!("访问令牌: {}", token.trim());

                    // if let Some(machine_id) = storage_json["telemetry.machineId"].as_str() {
                    //     println!("machineId: {}", machine_id);
                    // }

                    // if let Some(mac_machine_id) = storage_json["telemetry.macMachineId"].as_str() {
                    //     println!("macMachineId: {}", mac_machine_id);
                    // }

                    let sys_machine_id = get_machine_id();
                    println!("系统 machine-id: {}", sys_machine_id);

                    if let (Some(machine_id), Some(mac_machine_id)) = (
                        storage_json["telemetry.machineId"].as_str(),
                        storage_json["telemetry.macMachineId"].as_str(),
                    ) {
                        println!(
                            "校验和: {}{}/{}",
                            generate_timestamp_header(),
                            machine_id,
                            mac_machine_id
                        );
                    }
                }
                Err(err) => eprintln!("获取令牌时出错: {}", err),
            }
        }
        Err(err) => eprintln!("无法打开数据库: {}", err),
    }
}
