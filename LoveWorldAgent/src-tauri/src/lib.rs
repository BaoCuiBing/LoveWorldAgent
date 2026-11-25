pub mod agent; // 应用入口：解析命令行、初始化日志、启动本地 HTTP 服务并创建 Tauri 窗口
pub mod api; // 桌面原生能力路由
pub mod app; // HTTP 服务组装
pub mod config; // 配置加载与迁移
pub mod rouer; // 业务路由集合
pub mod state; // 全局应用状态
pub mod utils; // 工具模块
use serde_json::Value; // JSON 动态值
use std::sync::atomic::{AtomicBool, Ordering}; // 退出防重入标志
use tauri::window::Color; // 窗口背景色
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder}; // Tauri 窗口构造
use utils::logger; // 日志输出
static EXITING: AtomicBool = AtomicBool::new(false); // 窗口退出防重入标志
const APP_USER_MODEL_ID: &str = "com.loveworld.agent"; // 主窗口与内置浏览器共用的 Windows 任务栏分组标识
#[cfg(windows)]
mod win_taskbar {
    use std::ffi::c_void;
    use windows_sys::core::{GUID, HRESULT};
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows_sys::Win32::System::Variant::VT_LPWSTR;
    use windows_sys::Win32::UI::Shell::PropertiesSystem::{PROPERTYKEY, SHGetPropertyStoreForWindow};
    use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
    const IID_IPROPERTY_STORE: GUID = GUID::from_u128(0x886d8eeb_8cf2_4446_8d02_cdba1dbdcf99);
    const PKEY_APPUSERMODEL_ID: PROPERTYKEY = PROPERTYKEY {
        fmtid: GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
        pid: 5,
    };
    #[repr(C)]
    struct IPropertyStoreVtbl {
        query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
        add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
        get_count: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
        get_at: unsafe extern "system" fn(*mut c_void, u32, *mut PROPERTYKEY) -> HRESULT,
        get_value: unsafe extern "system" fn(*mut c_void, *const PROPERTYKEY, *mut PROPVARIANT) -> HRESULT,
        set_value: unsafe extern "system" fn(*mut c_void, *const PROPERTYKEY, *const PROPVARIANT) -> HRESULT,
        commit: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    }
    #[repr(C)]
    struct RawPropertyStore {
        lp_vtbl: *const IPropertyStoreVtbl,
    }
    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }
    pub fn set_process_aumid(aumid: &str) -> HRESULT {
        let mut w = wide(aumid);
        unsafe { SetCurrentProcessExplicitAppUserModelID(w.as_mut_ptr()) }
    }
    pub fn set_window_aumid(hwnd: isize, aumid: &str) -> HRESULT {
        if hwnd == 0 {
            return -1;
        }
        let mut store: *mut c_void = std::ptr::null_mut();
        let hr = unsafe { SHGetPropertyStoreForWindow(hwnd as HWND, &IID_IPROPERTY_STORE, &mut store) };
        if hr < 0 || store.is_null() {
            return hr;
        }
        let mut w = wide(aumid);
        unsafe {
            let mut pv: PROPVARIANT = std::mem::zeroed();
            pv.Anonymous.Anonymous.vt = VT_LPWSTR;
            pv.Anonymous.Anonymous.Anonymous.pwszVal = w.as_mut_ptr();
            let vtbl = &*(*(store as *const RawPropertyStore)).lp_vtbl;
            let set_hr = (vtbl.set_value)(store, &PKEY_APPUSERMODEL_ID, &pv);
            let commit_hr = if set_hr >= 0 { (vtbl.commit)(store) } else { set_hr };
            (vtbl.release)(store);
            // Keep `w` alive across set_value/commit/release; never call PropVariantClear on Rust Vec<u16>
            commit_hr
        }
    }
}
fn parse_color(hex: &str) -> Color { // 解析 #RRGGBB 为窗口颜色
    let text = hex.trim().trim_start_matches('#'); // 去 # 前缀
    let value = u32::from_str_radix(text, 16).unwrap_or(0x181818); // 十六进制解析，失败用默认
    Color(((value >> 16) & 0xff) as u8, ((value >> 8) & 0xff) as u8, (value & 0xff) as u8, 255) // 拆 RGB，alpha 固定 255
}
fn report_field(report: &Value, key: &str) -> String { // 读取迁移报告字段
    match report.get(key) { // 按 key 取值
        Some(Value::String(text)) => text.clone(), // 字符串
        Some(value) => value.to_string(), // 其他转字符串
        None => "-".to_string(), // 缺失用占位
    }
}
pub fn run() { // 应用主流程
    #[cfg(windows)]
    {
        let hr = win_taskbar::set_process_aumid(APP_USER_MODEL_ID);
        if hr < 0 {
            eprintln!("SetCurrentProcessExplicitAppUserModelID failed: 0x{:08x}", hr as u32);
        }
    }
    let args: Vec<String> = std::env::args().collect(); // 命令行参数
    let dev = args.iter().any(|arg| arg == "--dev"); // 是否开发模式
    let debug = args.iter().any(|arg| arg == "--debug") || std::env::var("WEBVIEW_DEBUG").map(|value| value == "1").unwrap_or(false); // 是否开启调试
    config::init(); // 初始化配置
    let skill_dir = config::WRITABLE_LOVEWORD_DIR.join("skills"); // 可写技能目录
    let _ = std::fs::create_dir_all(&skill_dir); // 确保技能目录存在
    agent::skills::seed_skills(&skill_dir, &config::LOVEWORD_DIR.join("skills")); // 复用播种，保证锁定技能已就位（不存在时 sync_locked 会跳过）
    let permission_data = agent::permission::load(); // 权限配置
    let computer = permission_data.get("groups").and_then(|groups| groups.get("computer")).and_then(|group| group.get("enabled")).and_then(Value::as_bool).unwrap_or(true); // 电脑控制总开关
    let word = permission_data.get("groups").and_then(|groups| groups.get("word")).and_then(|group| group.get("enabled")).and_then(Value::as_bool).unwrap_or(true); // Word 操作总开关
    let ppt = permission_data.get("groups").and_then(|groups| groups.get("ppt")).and_then(|group| group.get("enabled")).and_then(Value::as_bool).unwrap_or(true); // PPT 操作总开关
    let excel = permission_data.get("groups").and_then(|groups| groups.get("excel")).and_then(|group| group.get("enabled")).and_then(Value::as_bool).unwrap_or(true); // Excel 操作总开关
    let mobile = permission_data.get("groups").and_then(|groups| groups.get("mobile")).and_then(|group| group.get("enabled")).and_then(Value::as_bool).unwrap_or(true); // 移动端总开关
    utils::office_option::set_enabled("word", word); // 同步 Word 族开关
    utils::office_option::set_enabled("ppt", ppt); // 同步 PPT 族开关
    utils::office_option::set_enabled("excel", excel); // 同步 Excel 族开关
    agent::skills::sync_locked(&skill_dir, "browser", utils::browser::enabled()); // 启动时同步 browser-automation 与浏览器总开关
    agent::skills::sync_locked(&skill_dir, "computer", computer); // 启动时同步 computer-control 与电脑总开关
    agent::skills::sync_locked(&skill_dir, "word", word); // 启动时同步 word-control 与 Word 总开关
    agent::skills::sync_locked(&skill_dir, "ppt", ppt); // 启动时同步 ppt-control
    agent::skills::sync_locked(&skill_dir, "excel", excel); // 启动时同步 excel-control
    agent::skills::sync_locked(&skill_dir, "mobile", mobile); // 启动时同步 mobile-control
    logger::init(&config::LOG_DIR, &config::LOG_LEVEL, *config::LOG_MAX_LINES); // 初始化日志
    logger::info("main", &format!("配置来源: {}", config::CONFIG_FILE.as_ref().map(|path| path.display().to_string()).unwrap_or_else(|| "内置默认值".to_string()))); // 记录配置来源
    let report = &*config::MIGRATION_REPORT; // 迁移报告
    logger::info("main", &format!("智能体配置迁移: agent_config={} permission={} stripped={}", report_field(report, "agent_config"), report_field(report, "permission"), report_field(report, "stripped"))); // 记录迁移结果
    match app::start_server(&config::HTTP_HOST, *config::HTTP_PORT) { // 启动 HTTP 服务
        Ok(()) => {} // 成功
        Err(err) => logger::error("main", &format!("HTTP 服务启动失败: {err}")), // 失败记录
    }
    let url = if dev { (*config::DEV_SERVER_URL).clone() } else { format!("http://{}:{}/", *config::HTTP_HOST, *config::HTTP_PORT) }; // 窗口加载地址
    let icon = utils::icon::ensure_icon(); // 确保窗口图标
    logger::info("main", &format!("启动窗口（{}）: {url} 图标={}", if dev { "dev" } else { "prod" }, icon.as_ref().map(|path| path.display().to_string()).unwrap_or_else(|| "系统默认".to_string()))); // 记录启动信息
    let min_size = *config::WINDOW_MIN_SIZE; // 最小窗口尺寸
    tauri::Builder::default() // 构建 Tauri 应用
        .plugin(tauri_plugin_dialog::init()) // 对话框插件
        .plugin(tauri_plugin_notification::init()) // 通知插件
        .plugin(tauri_plugin_opener::init()) // 打开外部程序插件
        .setup(move |handle| { // 窗口创建完成回调
            let parsed = tauri::Url::parse(&url).map_err(|err| format!("窗口地址无效 {url}: {err}"))?; // 解析窗口 URL
            let window = WebviewWindowBuilder::new(handle, "main", WebviewUrl::External(parsed)) // 创建主窗口
                .title(config::WINDOW_TITLE.as_str()) // 标题
                .inner_size(*config::WINDOW_WIDTH, *config::WINDOW_HEIGHT) // 初始尺寸
                .min_inner_size(min_size.0, min_size.1) // 最小尺寸
                .background_color(parse_color(&config::BACKGROUND_COLOR)) // 背景色
                .data_directory((*config::WEBVIEW_STORAGE_DIR).clone()) // WebView2 用户数据固定到可写目录，避免系统缓存锁冲突
                .center() // 居中
                .devtools(debug) // 调试工具开关
                .build()?; // 构建窗口
            logger::info("main", &format!("窗口已创建: {} 标题={} 尺寸={}x{}", url, window.title().unwrap_or_default(), *config::WINDOW_WIDTH, *config::WINDOW_HEIGHT)); // 记录
            crate::state::set_handle(handle.handle().clone()); // 保存应用句柄
            #[cfg(windows)]
            match window.hwnd() { // 为主窗口设置与内置浏览器一致的 AppUserModelID，实现任务栏双窗分组重叠
                Ok(hwnd) => {
                    let hr = win_taskbar::set_window_aumid(hwnd.0 as isize, APP_USER_MODEL_ID);
                    if hr < 0 { logger::warn("main", &format!("设置主窗口 AppUserModelID 失败: 0x{:08x}", hr as u32)); }
                }
                Err(err) => logger::warn("main", &format!("获取主窗口句柄失败，无法设置 AppUserModelID: {err}")),
            }
            crate::rouer::terminal_router::start_reaper(); // 启动终端回收线程
            crate::agent::mcp::spawn_startup_connect(); // 后台常驻连接所有启用的 MCP 服务，不阻塞窗口
            let theme = config::get_str(&config::CONFIG, &["ui", "theme"], "dark"); // 读取主题设置
            logger::info("main", &format!("标题栏配色: {}", crate::utils::titlebar::apply(&theme, None))); // 应用标题栏主题
            crate::utils::menu::apply(handle.handle()); // 创建原生菜单
            Ok(()) // 初始化完成
        })
        .on_window_event(|window, event| { // 窗口事件回调
            if let tauri::WindowEvent::CloseRequested { api, .. } = event { // 关闭请求：先拦截并通知前端展示 QuitState 弹窗
                api.prevent_close(); // 阻止同步关闭卡死 UI 线程
                if EXITING.swap(true, Ordering::SeqCst) { return; } // 已在退出流程中则忽略重复点击
                logger::info("main", "收到关闭请求，通知前端展示退出中弹窗并异步清理服务"); // 记录
                let dispatched = window.app_handle().get_webview_window(window.label()).is_some_and(|view| view.eval("window.dispatchEvent(new Event('lwa:quit-requested'))").is_ok()); // 经 WebviewWindow 派发前端退出弹窗事件
                if !dispatched {
                    let app = window.app_handle().clone(); // 前端不可用时直接后台限时清理退出
                    tauri::async_runtime::spawn(async move {
                        rouer::other_router::cleanup_services(&state::shared()).await;
                        app.exit(0);
                        std::process::exit(0);
                    });
                }
            }
        })
        .run(tauri::generate_context!()) // 运行应用
        .expect("LoveWorldAgent 启动失败"); // 启动失败则 panic
}
