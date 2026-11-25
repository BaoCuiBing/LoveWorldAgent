use std::path::PathBuf; // 本机 Chrome/Edge Cookie 读取：DPAPI 取密钥、AES-256-GCM 解密 v10/v11，v20 标记跳过
use aes_gcm::aead::Aead; // AEAD 解密 trait
use aes_gcm::{Aes256Gcm, Key, KeyInit, Nonce}; // AES-GCM 类型
use base64::Engine; // Base64 解码
use rusqlite::Connection; // SQLite 连接
use serde_json::{json, Map, Value}; // JSON 构造
use windows_sys::Win32::Foundation::LocalFree; // 释放 DPAPI 内存
use windows_sys::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB}; // Windows DPAPI
use crate::utils::logger; // 日志输出
pub struct Source { // 可导入的浏览器来源
    pub key: &'static str, // 来源键
    pub name: &'static str, // 显示名
}
pub const SOURCES: [Source; 2] = [ // 支持的浏览器列表
    Source { key: "chrome", name: "Google Chrome" }, // Chrome
    Source { key: "edge", name: "Microsoft Edge" }, // Edge
];
fn home() -> PathBuf { // 取用户主目录
    std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(".")) // Windows 优先 USERPROFILE
}
pub fn source_root(key: &str) -> Option<PathBuf> { // 取浏览器用户数据目录
    let base = home().join("AppData").join("Local"); // LocalAppData
    match key { // 按来源分派
        "chrome" => Some(base.join("Google").join("Chrome").join("User Data")), // Chrome
        "edge" => Some(base.join("Microsoft").join("Edge").join("User Data")), // Edge
        _ => None, // 未知来源
    }
}
pub fn list_sources() -> Value { // 列出来源及可用性
    let items: Vec<Value> = SOURCES.iter().map(|source| { // 逐来源
        let available = source_root(source.key).map(|root| root.join("Local State").is_file()).unwrap_or(false); // 有 Local State 即视为可用
        json!({"key": source.key, "name": source.name, "available": available}) // 来源信息
    }).collect();
    Value::Array(items) // 返回数组
}
fn profiles(root: &PathBuf) -> Vec<PathBuf> { // 列举浏览器配置目录
    let mut result = Vec::new(); // 结果
    let default = root.join("Default"); // 默认配置
    if default.is_dir() { result.push(default); } // 默认配置优先
    if let Ok(entries) = std::fs::read_dir(root) { // 遍历其它配置
        let mut others: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).filter(|path| path.is_dir() && path.file_name().map(|name| name.to_string_lossy().starts_with("Profile ")).unwrap_or(false)).collect(); // Profile N 目录
        others.sort(); // 排序
        result.extend(others); // 追加
    }
    result // 返回配置列表
}
fn decrypt_key(root: &PathBuf) -> Result<Vec<u8>, String> { // 用 DPAPI 解密浏览器 Cookie 密钥
    let text = std::fs::read_to_string(root.join("Local State")).map_err(|err| err.to_string())?; // 读取 Local State
    let payload = serde_json::from_str::<Value>(&text).map_err(|err| err.to_string())?; // 解析 JSON
    let encoded = payload.get("os_crypt").and_then(|value| value.get("encrypted_key")).and_then(Value::as_str).ok_or_else(|| "Local State 缺少 encrypted_key".to_string())?; // 取加密密钥
    let raw = base64::engine::general_purpose::STANDARD.decode(encoded).map_err(|err| err.to_string())?; // Base64 解码
    if !raw.starts_with(b"DPAPI") { return Err("未知的密钥格式（非 DPAPI 前缀）".to_string()); } // 格式校验
    let blob = &raw[5..]; // 去掉 DPAPI 前缀
    unsafe { // 调用 Windows DPAPI
        let mut input = CRYPT_INTEGER_BLOB { cbData: blob.len() as u32, pbData: blob.as_ptr() as *mut u8 }; // 输入 blob
        let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() }; // 输出 blob
        let ok = CryptUnprotectData(&mut input, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), 0, &mut output); // 解密
        if ok == 0 { return Err("DPAPI 解密浏览器密钥失败".to_string()); } // 失败
        let key = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec(); // 复制密钥
        LocalFree(output.pbData as _); // 释放系统内存
        Ok(key) // 返回密钥
    }
}
fn decrypt_value(key: &[u8], blob: &[u8]) -> Option<String> { // 用 AES-GCM 解密 Cookie 值
    if blob.len() < 15 || key.len() != 32 { return None; } // 长度校验
    if blob.starts_with(b"v20") { return None; } // v20 不支持
    if !(blob.starts_with(b"v10") || blob.starts_with(b"v11")) { return None; } // 仅 v10/v11
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key)); // 构造密码器
    let nonce = Nonce::from_slice(&blob[3..15]); // 取 nonce
    cipher.decrypt(nonce, &blob[15..]).ok().map(|bytes| String::from_utf8_lossy(&bytes).to_string()) // 解密并解码
}
fn count_skip(skipped: &mut Map<String, Value>, reason: &str) { // 累加跳过原因计数
    let current = skipped.get(reason).and_then(Value::as_u64).unwrap_or(0); // 当前计数
    skipped.insert(reason.to_string(), json!(current + 1)); // 加一
}
pub fn read_cookies(source: &str, include_expired: bool) -> Result<Value, String> { // 读取指定来源的 Cookie
    let info = SOURCES.iter().find(|item| item.key == source).ok_or_else(|| format!("未知来源: {source}"))?; // 查找来源
    let root = source_root(info.key).ok_or_else(|| format!("未知来源: {source}"))?; // 取数据目录
    if !root.join("Local State").is_file() { return Err(format!("{} 未安装或从未运行（缺少 Local State）", info.name)); } // 未安装
    let key = decrypt_key(&root)?; // 解密密钥
    let mut cookies: Vec<Value> = Vec::new(); // Cookie 结果
    let mut skipped = Map::new(); // 跳过统计
    let mut errors: Vec<Value> = Vec::new(); // 错误列表
    let mut scanned_profiles: Vec<String> = Vec::new(); // 已扫描配置
    let mut scanned = 0u64; // 扫描计数
    let now_us = now_utc_micro(); // 当前时间（1601 纪元微秒）
    for profile in profiles(&root) { // 遍历配置
        let database = profile.join("Network").join("Cookies"); // Cookie 数据库
        if !database.is_file() { continue; } // 不存在跳过
        let temp = std::env::temp_dir().join(format!("lwa_cookies_{}.db", profile.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_else(|| "default".to_string()))); // 临时副本路径
        if let Err(err) = std::fs::copy(&database, &temp) { // 复制数据库避免锁库
            count_skip(&mut skipped, "read"); // 统计跳过
            errors.push(json!({"profile": profile.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default(), "reason": "read", "message": err.to_string()})); // 记录错误
            continue; // 下一配置
        }
        let result = read_profile(&temp, include_expired, now_us, &key, &mut cookies, &mut skipped, &mut scanned); // 读取配置 Cookie
        let _ = std::fs::remove_file(&temp); // 清理临时文件
        if let Err(err) = result { // 读取失败
            count_skip(&mut skipped, "read"); // 统计跳过
            errors.push(json!({"profile": profile.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default(), "reason": "read", "message": err.to_string()})); // 记录错误
            continue; // 下一配置
        }
        scanned_profiles.push(profile.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default()); // 记录已扫描
    }
    logger::info("systeam_browser", &format!("Cookie 导入扫描: source={source} total={scanned} skipped={skipped:?} profiles={scanned_profiles:?}")); // 记录扫描结果
    Ok(json!({"cookies": cookies, "total": cookies.len(), "scanned": scanned, "skipped": skipped, "profiles": scanned_profiles, "errors": errors})) // 返回结果
}
type CookieRow = (String, String, Option<String>, Option<Vec<u8>>, Option<String>, Option<i64>, Option<i64>, Option<i64>, Option<i64>); // Cookie 行元组类型
fn read_profile(database: &PathBuf, include_expired: bool, now_us: i64, key: &[u8], cookies: &mut Vec<Value>, skipped: &mut Map<String, Value>, scanned: &mut u64) -> Result<(), String> { // 读取单个配置的 Cookie
    let conn = Connection::open(database).map_err(|err| err.to_string())?; // 打开数据库
    let full = "SELECT host_key, name, value, encrypted_value, path, expires_utc, is_secure, is_httponly, samesite FROM cookies"; // 完整查询
    let rows: Vec<CookieRow> = match conn.prepare(full) { // 尝试完整查询
        Ok(mut statement) => { // 新版本表结构
            let mapped = statement.query_map([], |row| Ok(( // 逐行映射
                row.get::<_, String>(0).unwrap_or_default(), // host
                row.get::<_, String>(1).unwrap_or_default(), // name
                row.get::<_, Option<String>>(2).unwrap_or_default(), // value
                row.get::<_, Option<Vec<u8>>>(3).unwrap_or_default(), // encrypted_value
                row.get::<_, Option<String>>(4).unwrap_or_default(), // path
                row.get::<_, Option<i64>>(5).unwrap_or_default(), // expires
                row.get::<_, Option<i64>>(6).unwrap_or_default(), // secure
                row.get::<_, Option<i64>>(7).unwrap_or_default(), // httponly
                row.get::<_, Option<i64>>(8).unwrap_or_default(), // samesite
            ))).map_err(|err| err.to_string())?; // 查询失败
            mapped.flatten().collect() // 收集行
        }
        Err(_) => { // 旧版本表结构，缺 samesite 等列
            let mut statement = conn.prepare("SELECT host_key, name, value, encrypted_value, path, expires_utc, is_secure FROM cookies").map_err(|err| err.to_string())?; // 降级查询
            let mapped = statement.query_map([], |row| Ok(( // 逐行映射
                row.get::<_, String>(0).unwrap_or_default(), // host
                row.get::<_, String>(1).unwrap_or_default(), // name
                row.get::<_, Option<String>>(2).unwrap_or_default(), // value
                row.get::<_, Option<Vec<u8>>>(3).unwrap_or_default(), // encrypted_value
                row.get::<_, Option<String>>(4).unwrap_or_default(), // path
                row.get::<_, Option<i64>>(5).unwrap_or_default(), // expires
                row.get::<_, Option<i64>>(6).unwrap_or_default(), // secure
                None, // httponly 缺失
                Some(-1i64), // samesite 缺失
            ))).map_err(|err| err.to_string())?; // 查询失败
            mapped.flatten().collect() // 收集行
        }
    };
    for row in rows { // 逐 Cookie 处理
        *scanned += 1; // 扫描计数
        let (host, name, value, encrypted, path, expires, secure, httponly, samesite) = row; // 解构行
        if let Some(stamp) = expires { // 有过期时间
            if stamp != 0 && !include_expired && stamp < now_us { count_skip(skipped, "expired"); continue; } // 已过期跳过
        }
        let clear = match value { // 取明文值
            Some(text) if !text.is_empty() => text, // 明文字段
            _ => match encrypted { // 否则解密
                Some(blob) if blob.starts_with(b"v20") => { count_skip(skipped, "v20"); continue; } // v20 跳过
                Some(blob) => match decrypt_value(key, &blob) { // 解密
                    Some(text) => text, // 成功
                    None => { count_skip(skipped, "decrypt"); continue; } // 失败跳过
                },
                _ => { count_skip(skipped, "decrypt"); continue; } // 无值跳过
            },
        };
        let scheme = if secure.unwrap_or(0) != 0 { "https" } else { "http" }; // 协议
        let path_text = path.unwrap_or_default(); // 路径
        let path_text = if path_text.is_empty() { "/".to_string() } else { path_text }; // 空路径用根
        cookies.push(json!({"url": format!("{scheme}://{host}{path_text}"), "name": name, "value": clear, "domain": host, "path": path_text, "secure": secure.unwrap_or(0) != 0, "httponly": httponly.unwrap_or(0) != 0, "samesite": samesite.unwrap_or(-1)})); // 写入 Cookie
    }
    Ok(()) // 完成
}
fn now_utc_micro() -> i64 { // 当前时间转 1601 纪元微秒
    let now = chrono::Utc::now(); // 当前 UTC
    let epoch = chrono::DateTime::parse_from_rfc3339("1601-01-01T00:00:00Z").map(|value| value.timestamp_micros()).unwrap_or(0); // Chrome 纪元
    now.timestamp_micros() - epoch // 差值
}
