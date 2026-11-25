pub mod actions; // 键鼠输入、窗口控制与菜单动作工具
pub mod agent_api; // 工具定义与调度总入口（严格 23 个 computer_* 工具）
pub mod apps; // 应用与窗口枚举、启停、置前、几何及系统探测工具
pub mod pipe_client; // 命名管道客户端
pub mod snapshots; // 窗口 UIA 快照登记表与 element_token 解析
pub mod state_tools; // get_window_state 与 get_desktop_state（omni_parser 视觉解析）