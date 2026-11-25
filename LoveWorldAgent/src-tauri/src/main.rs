#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // 发布构建隐藏控制台窗口
fn main() { // 二进制入口
    loveworldagent::run(); // 委托库入口
}
