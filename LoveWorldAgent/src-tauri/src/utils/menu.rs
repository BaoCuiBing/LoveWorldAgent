use tauri::menu::{CheckMenuItem, CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, SubmenuBuilder}; // 原生菜单：文件（新建对话/退出）与显示（置顶/刷新），等价 menu.py
use tauri::{AppHandle, Manager, Wry}; // 应用句柄与窗口管理
use crate::utils::logger; // 日志输出
pub const MENU_NEW_CHAT: &str = "lwa_new_chat"; // 新建对话菜单 id
pub const MENU_QUIT: &str = "lwa_quit"; // 退出菜单 id
pub const MENU_ON_TOP: &str = "lwa_on_top"; // 置顶菜单 id
pub const MENU_REFRESH: &str = "lwa_refresh"; // 刷新菜单 id
pub const MENU_DEVTOOLS: &str = "lwa_devtools"; // 开发者工具菜单 id
pub const MENU_CLEAR_DATA: &str = "lwa_clear_data"; // 清除数据菜单 id
pub fn apply(app: &AppHandle) -> bool { // 构建并安装原生菜单
    let built = (|| -> tauri::Result<()> { // 构建过程，便于统一错误处理
        let new_chat = MenuItemBuilder::with_id(MENU_NEW_CHAT, "新建对话").build(app)?; // 新建对话项
        let quit = MenuItemBuilder::with_id(MENU_QUIT, "退出").build(app)?; // 退出项
        let file = SubmenuBuilder::new(app, "文件").item(&new_chat).separator().item(&quit).build()?; // 文件子菜单
        let on_top = CheckMenuItemBuilder::with_id(MENU_ON_TOP, "置顶").checked(false).build(app)?; // 置顶勾选项
        let refresh = MenuItemBuilder::with_id(MENU_REFRESH, "刷新").build(app)?; // 刷新项
        let display = SubmenuBuilder::new(app, "显示").item(&on_top).separator().item(&refresh).build()?; // 显示子菜单
        let devtools = MenuItemBuilder::with_id(MENU_DEVTOOLS, "开发者工具").build(app)?; // 开发者工具项
        let clear_data = MenuItemBuilder::with_id(MENU_CLEAR_DATA, "清除数据").build(app)?; // 清除数据项
        let tools = SubmenuBuilder::new(app, "工具").item(&devtools).separator().item(&clear_data).build()?; // 工具子菜单
        let menu = MenuBuilder::new(app).item(&file).item(&display).item(&tools).build()?; // 组装菜单
        if let Some(window) = app.get_webview_window("main") { window.set_menu(menu)?; } // 安装到主窗口
        let check = on_top.clone(); // 克隆勾选项供事件闭包
        app.on_menu_event(move |app, event| { // 注册菜单事件监听
            let id: &str = event.id().as_ref(); // 取菜单 id
            handle_event(app, &check, id); // 分派处理
        });
        Ok(()) // 构建成功
    })();
    match built { // 处理结果
        Ok(()) => { // 成功
            logger::info("menu", "原生菜单已创建"); // 记录
            true // 返回成功
        }
        Err(err) => { // 失败
            logger::warn("menu", &format!("创建原生菜单失败: {err}")); // 记录告警
            false // 返回失败
        }
    }
}
fn eval(app: &AppHandle, script: &str) { // 在主窗口执行前端脚本
    let Some(window) = app.get_webview_window("main") else { return; }; // 无窗口则跳过
    if let Err(err) = window.eval(script) { logger::warn("menu", &format!("执行前端脚本失败: {err}")); } // 执行失败记录
}
fn handle_event(app: &AppHandle, on_top: &CheckMenuItem<Wry>, id: &str) { // 分派菜单点击事件
    match id { // 按菜单 id 分派
        MENU_NEW_CHAT => eval(app, "window.dispatchEvent(new Event('lwa:new-chat'))"), // 触发新建对话
        MENU_REFRESH => eval(app, "location.reload()"), // 刷新页面
        MENU_DEVTOOLS => { // 打开开发者工具（开发态与发布态均可用）
            if let Some(window) = app.get_webview_window("main") { window.open_devtools(); } // 打开 WebView2 DevTools
        }
        MENU_CLEAR_DATA => eval(app, "window.dispatchEvent(new Event('lwa:clear-data'))"), // 触发清除数据弹窗
        MENU_QUIT => { // 退出应用：交由窗口关闭流程展示 QuitState 弹窗并清理后台服务
            if let Some(window) = app.get_webview_window("main") { let _ = window.close(); } else { app.exit(0); }
        }
        MENU_ON_TOP => { // 切换置顶
            let Some(window) = app.get_webview_window("main") else { return; }; // 无窗口则跳过
            let next = !window.is_always_on_top().unwrap_or(false); // 取反当前状态
            if let Err(err) = window.set_always_on_top(next) { logger::warn("menu", &format!("切换置顶失败: {err}")); } // 设置置顶
            if let Err(err) = on_top.set_checked(next) { logger::warn("menu", &format!("同步置顶勾选态失败: {err}")); } // 同步勾选态
        }
        _ => {} // 未知 id 忽略
    }
}
pub fn apply_theme(theme: &str) -> bool { // 菜单主题（原生菜单跟随系统，不做逐项刷色）
    logger::info("menu", &format!("菜单配色: theme={theme}（Tauri 原生菜单跟随系统主题，不做逐项刷色）")); // 记录
    true // 返回成功
}
