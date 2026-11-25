pub mod context; // store_sqls 子模块：公共层、会话、消息、轮次、用量、上下文摘要
pub mod db; // 公共层：库路径、建表、时间格式与行转换
pub mod messages; // 消息表存取：追加、替换与分页
pub mod sessions; // 会话表 CRUD 与级联删除
pub mod turns; // 轮次表：一轮含用户与助手消息
pub mod usage; // token 用量累加与读取
