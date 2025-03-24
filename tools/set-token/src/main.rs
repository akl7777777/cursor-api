use rusqlite::{Connection, Result};
use serde_json::{Value, from_str, to_string_pretty};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

fn get_cursor_path() -> PathBuf {
    let home = if cfg!(windows) {
        env::var("USERPROFILE").unwrap_or_else(|_| env::var("HOME").unwrap())
    } else {
        env::var("HOME").unwrap()
    };

    let base_path = PathBuf::from(home);

    if cfg!(windows) {
        base_path.join("AppData\\Roaming\\Cursor")
    } else if cfg!(target_os = "macos") {
        base_path.join("Library/Application Support/Cursor")
    } else {
        base_path.join(".config/Cursor")
    }
}

fn update_sqlite_tokens(
    refresh_token: &str,
    access_token: &str,
    email: &str,
    signup_type: &str,
    membership_type: &str,
) -> Result<()> {
    let db_path = get_cursor_path().join("User/globalStorage/state.vscdb");
    let conn = Connection::open(db_path)?;

    // 获取原始值
    let mut stmt = conn.prepare(
        "SELECT key, value FROM ItemTable WHERE key IN (
            'cursorAuth/refreshToken',
            'cursorAuth/accessToken',
            'cursorAuth/cachedEmail',
            'cursorAuth/cachedSignUpType',
            'cursorAuth/stripeMembershipType'
        )",
    )?;

    println!("\n原始值：");
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (key, value) = row?;
        println!("{}: {}", key, value);
    }

    // 更新值
    conn.execute(
        "UPDATE ItemTable SET value = ? WHERE key = 'cursorAuth/refreshToken'",
        [refresh_token],
    )?;
    conn.execute(
        "UPDATE ItemTable SET value = ? WHERE key = 'cursorAuth/accessToken'",
        [access_token],
    )?;
    conn.execute(
        "UPDATE ItemTable SET value = ? WHERE key = 'cursorAuth/cachedEmail'",
        [email],
    )?;
    conn.execute(
        "UPDATE ItemTable SET value = ? WHERE key = 'cursorAuth/cachedSignUpType'",
        [signup_type],
    )?;
    conn.execute(
        "UPDATE ItemTable SET value = ? WHERE key = 'cursorAuth/stripeMembershipType'",
        [membership_type],
    )?;

    println!("\n更新后的值：");
    let mut stmt = conn.prepare(
        "SELECT key, value FROM ItemTable WHERE key IN (
            'cursorAuth/refreshToken',
            'cursorAuth/accessToken',
            'cursorAuth/cachedEmail',
            'cursorAuth/cachedSignUpType',
            'cursorAuth/stripeMembershipType'
        )",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (key, value) = row?;
        println!("{}: {}", key, value);
    }

    Ok(())
}

fn update_storage_json(machine_ids: &[String; 4]) -> io::Result<()> {
    let storage_path = get_cursor_path().join("User/globalStorage/storage.json");
    let content = fs::read_to_string(&storage_path)?;
    let mut json: Value = from_str(&content)?;

    if let Value::Object(ref mut map) = json {
        map.insert(
            "telemetry.macMachineId".to_string(),
            Value::String(machine_ids[0].clone()),
        );
        map.insert(
            "telemetry.sqmId".to_string(),
            Value::String(machine_ids[1].clone()),
        );
        map.insert(
            "telemetry.machineId".to_string(),
            Value::String(machine_ids[2].clone()),
        );
        map.insert(
            "telemetry.devDeviceId".to_string(),
            Value::String(machine_ids[3].clone()),
        );
    }

    fs::write(storage_path, to_string_pretty(&json)?)?;
    Ok(())
}

fn is_valid_jwt(token: &str) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        println!("警告: Token 格式不正确，应该包含3个由'.'分隔的部分");
        return false;
    }

    // 检查是否以 "ey" 开头
    if !token.starts_with("ey") {
        println!("警告: Token 应该以'ey'开头");
        return false;
    }

    true
}

fn is_valid_sha256(id: &str) -> bool {
    // SHA256 哈希是64个十六进制字符
    if id.len() != 64 {
        println!("警告: ID 长度应为64个字符");
        return false;
    }

    // 检查是否都是有效的十六进制字符
    if !id.chars().all(|c| c.is_ascii_hexdigit()) {
        println!("警告: ID 应只包含十六进制字符(0-9, a-f)");
        return false;
    }

    true
}

fn is_valid_sqm_id(id: &str) -> bool {
    // 格式应为 {XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX} (大写)
    if id.len() != 38 {
        println!("警告: SQM ID 格式不正确");
        return false;
    }

    if !id.starts_with('{') || !id.ends_with('}') {
        println!("警告: SQM ID 应该被花括号包围");
        return false;
    }

    let uuid = &id[1..37];
    if !uuid
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-')
    {
        println!("警告: UUID 部分应为大写字母、数字和连字符");
        return false;
    }

    true
}

fn is_valid_device_id(id: &str) -> bool {
    // 格式应为 xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    if id.len() != 36 {
        println!("警告: Device ID 格式不正确");
        return false;
    }

    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        println!("警告: Device ID 应为小写字母、数字和连字符");
        return false;
    }

    true
}

fn is_valid_email(email: &str) -> bool {
    if !email.contains('@') || !email.contains('.') {
        println!("警告: 邮箱格式不正确");
        return false;
    }
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        println!("警告: 邮箱格式不正确");
        return false;
    }
    true
}

fn is_valid_uuid(uuid: &str) -> bool {
    // UUID格式应为: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    if uuid.len() != 36 {
        println!("警告: UUID 格式不正确");
        return false;
    }

    let parts: Vec<&str> = uuid.split('-').collect();
    if parts.len() != 5
        || parts[0].len() != 8
        || parts[1].len() != 4
        || parts[2].len() != 4
        || parts[3].len() != 4
        || parts[4].len() != 12
    {
        println!("警告: UUID 格式不正确");
        return false;
    }

    if !uuid.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        println!("警告: UUID 应只包含十六进制字符(0-9, a-f)和连字符");
        return false;
    }

    true
}

fn create_uuid_launcher(uuid: &str) -> io::Result<()> {
    let tools_dir = get_cursor_path().join("tools/set-token");
    fs::create_dir_all(&tools_dir)?;

    // 创建 inject.js
    let inject_js = format!(
        r#"// 保存原始 require
const originalRequire = module.constructor.prototype.require;

// 重写 require 函数
module.constructor.prototype.require = function(path) {{
    const result = originalRequire.apply(this, arguments);
    
    // 检测目标模块
    if (path.includes('main.js')) {{
        // 保存原始函数
        const originalModule = result;
        
        // 创建代理对象
        return new Proxy(originalModule, {{
            get(target, prop) {{
                // 拦截 execSync 调用
                if (prop === 'execSync') {{
                    return function() {{
                        // 返回自定义的 UUID
                        const platform = process.platform;
                        switch (platform) {{
                            case 'darwin':
                                return 'IOPlatformUUID="{}"';
                            case 'win32':
                                return '    HARDWARE\\DESCRIPTION\\System\\BIOS    SystemProductID    REG_SZ    {}';
                            case 'linux':
                            case 'freebsd':
                                return '{}';
                            default:
                                throw new Error(`Unsupported platform: ${{platform}}`);
                        }}
                    }};
                }}
                return target[prop];
            }}
        }});
    }}
    return result;
}};"#,
        uuid, uuid, uuid
    );

    // 写入 inject.js
    fs::write(tools_dir.join("inject.js"), inject_js)?;

    if cfg!(windows) {
        // 创建 Windows CMD 脚本
        let cmd_script = format!(
            "@echo off\r\n\
            set NODE_OPTIONS=--require \"%~dp0inject.js\"\r\n\
            start \"\" \"%LOCALAPPDATA%\\Programs\\Cursor\\Cursor.exe\""
        );
        fs::write(tools_dir.join("start-cursor.cmd"), cmd_script)?;

        // 创建 Windows PowerShell 脚本
        let ps_script = format!(
            "$env:NODE_OPTIONS = \"--require `\"$PSScriptRoot\\inject.js`\"\"\r\n\
            Start-Process -FilePath \"$env:LOCALAPPDATA\\Programs\\Cursor\\Cursor.exe\""
        );
        fs::write(tools_dir.join("start-cursor.ps1"), ps_script)?;
    } else {
        // 创建 Shell 脚本
        let shell_script = format!(
            "#!/bin/bash\n\
            SCRIPT_DIR=\"$(cd \"$(dirname \"${{BASH_SOURCE[0]}}\")\" && pwd)\"\n\
            export NODE_OPTIONS=\"--require $SCRIPT_DIR/inject.js\"\n\
            if [[ \"$OSTYPE\" == \"darwin\"* ]]; then\n\
                open -a Cursor\n\
            else\n\
                cursor # Linux，根据实际安装路径调整\n\
            fi"
        );
        let script_path = tools_dir.join("start-cursor.sh");
        fs::write(&script_path, shell_script)?;

        // 在类Unix系统上设置可执行权限
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms)?;
        }
    }

    println!("\n注入脚本已创建在: {}", tools_dir.display());
    println!("\n使用方法:");
    if cfg!(windows) {
        println!(
            "方法1: 双击运行 {}",
            tools_dir.join("start-cursor.cmd").display()
        );
        println!(
            "方法2: 在 PowerShell 中运行 {}",
            tools_dir.join("start-cursor.ps1").display()
        );
    } else {
        println!(
            "在终端中运行: {}",
            tools_dir.join("start-cursor.sh").display()
        );
    }
    println!("\n注意：每次启动 Cursor 时都需要使用这个脚本。");

    Ok(())
}

fn main() {
    loop {
        println!("\n请选择操作：");
        println!("0. 退出");
        println!("1. 更新 Token");
        println!("2. 更新设备 ID");
        println!("3. 创建自定义UUID启动脚本");

        print!("请输入选项 (0-3): ");
        io::stdout().flush().unwrap();

        let mut choice = String::new();
        io::stdin().read_line(&mut choice).unwrap();

        match choice.trim() {
            "0" => break,
            "1" => {
                let mut refresh_token = String::new();
                loop {
                    print!("请输入 Refresh Token: ");
                    io::stdout().flush().unwrap();
                    refresh_token.clear();
                    io::stdin().read_line(&mut refresh_token).unwrap();
                    refresh_token = refresh_token.trim().to_string();

                    if is_valid_jwt(&refresh_token) {
                        break;
                    }
                    println!("请重新输入正确格式的 Token");
                }

                print!("Access Token 是否与 Refresh Token 相同? (y/n): ");
                io::stdout().flush().unwrap();
                let mut same = String::new();
                io::stdin().read_line(&mut same).unwrap();

                let access_token = if same.trim().eq_ignore_ascii_case("y") {
                    refresh_token.clone()
                } else {
                    let mut access_token = String::new();
                    loop {
                        print!("请输入 Access Token: ");
                        io::stdout().flush().unwrap();
                        access_token.clear();
                        io::stdin().read_line(&mut access_token).unwrap();
                        access_token = access_token.trim().to_string();

                        if is_valid_jwt(&access_token) {
                            break;
                        }
                        println!("请重新输入正确格式的 Token");
                    }
                    access_token
                };

                let mut email = String::new();
                loop {
                    print!("请输入邮箱: ");
                    io::stdout().flush().unwrap();
                    email.clear();
                    io::stdin().read_line(&mut email).unwrap();
                    email = email.trim().to_string();

                    if is_valid_email(&email) {
                        break;
                    }
                    println!("请重新输入正确格式的邮箱");
                }

                let mut signup_type = String::new();
                loop {
                    println!("\n可选的注册类型：");
                    println!("1. Auth_0");
                    println!("2. Github");
                    println!("3. Google");
                    println!("4. unknown");
                    println!("(WorkOS - 仅供展示，不可选择)");
                    print!("请选择注册类型 (1-4): ");
                    io::stdout().flush().unwrap();
                    signup_type.clear();
                    io::stdin().read_line(&mut signup_type).unwrap();

                    let signup_type_str = match signup_type.trim() {
                        "1" => "Auth_0",
                        "2" => "Github",
                        "3" => "Google",
                        "4" => "unknown",
                        _ => continue,
                    }
                    .to_string();

                    signup_type = signup_type_str;
                    break;
                }

                let mut membership_type = String::new();
                loop {
                    println!("\n可选的会员类型：");
                    println!("1. free");
                    println!("2. pro");
                    println!("3. enterprise");
                    println!("4. free_trial");
                    print!("请选择会员类型 (1-4): ");
                    io::stdout().flush().unwrap();
                    membership_type.clear();
                    io::stdin().read_line(&mut membership_type).unwrap();

                    let membership_type_str = match membership_type.trim() {
                        "1" => "free",
                        "2" => "pro",
                        "3" => "enterprise",
                        "4" => "free_trial",
                        _ => continue,
                    }
                    .to_string();

                    membership_type = membership_type_str;
                    break;
                }

                match update_sqlite_tokens(
                    &refresh_token,
                    &access_token,
                    &email,
                    &signup_type,
                    &membership_type,
                ) {
                    Ok(_) => println!("所有信息更新成功！"),
                    Err(e) => println!("更新失败: {}", e),
                }
            }
            "2" => {
                let mut ids = Vec::new();
                let validators: [(Box<dyn Fn(&str) -> bool>, &str); 4] = [
                    (Box::new(is_valid_sha256), "macMachineId"),
                    (Box::new(is_valid_sqm_id), "sqmId"),
                    (Box::new(is_valid_sha256), "machineId"),
                    (Box::new(is_valid_device_id), "devDeviceId"),
                ];

                for (validator, name) in validators.iter() {
                    loop {
                        print!("请输入 {}: ", name);
                        io::stdout().flush().unwrap();
                        let mut id = String::new();
                        io::stdin().read_line(&mut id).unwrap();
                        let id = id.trim().to_string();

                        if validator(&id) {
                            ids.push(id);
                            break;
                        }
                        println!("请重新输入正确格式的 ID");
                    }
                }

                match update_storage_json(&ids.try_into().unwrap()) {
                    Ok(_) => println!("设备 ID 更新成功！"),
                    Err(e) => println!("更新失败: {}", e),
                }
            }
            "3" => {
                let mut uuid = String::new();
                loop {
                    print!("请输入自定义 UUID (格式: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx): ");
                    io::stdout().flush().unwrap();
                    uuid.clear();
                    io::stdin().read_line(&mut uuid).unwrap();
                    uuid = uuid.trim().to_string();

                    if is_valid_uuid(&uuid) {
                        break;
                    }
                    println!("请重新输入正确格式的 UUID");
                }

                match create_uuid_launcher(&uuid) {
                    Ok(_) => println!("启动脚本创建成功！"),
                    Err(e) => println!("创建失败: {}", e),
                }
            }
            _ => println!("无效选项，请重试"),
        }
    }
}
