#include "bcb_check_box.h" // 引入自身头文件
#include "utils/config.h" // 引入配置工具类（用于资源路径解析）
BcbCheckBox::BcbCheckBox(QWidget* parent) : BcbCheckBox(QString(), parent) {} // 默认构造函数：委托给带文本构造函数
BcbCheckBox::BcbCheckBox(const QString& text, QWidget* parent) : QCheckBox(text, parent) { // 构造函数：设置勾选图标样式
    QString gouPath = Config::resolvePath("resources/icons/gou_xuan.png"); // 解析勾选图标路径
    m_compactStyle = QString(R"(QCheckBox { margin:0;font-size:11px;color:#424242;spacing:2px; }QCheckBox::indicator { width:14px;height:14px;border-radius:0;border:1px solid #E0E0E0;background:white; }QCheckBox::indicator:checked { border:1px solid #4CAF50;image:url(%1); }QCheckBox::indicator:hover { border:1px solid #81C784; })").arg(gouPath); // 紧凑样式
    setStyleSheet(QString(R"(QCheckBox { font-size:13px;color:#424242;spacing:8px; }QCheckBox::indicator { width:16px;height:16px;border-radius:0;border:2px solid #E0E0E0;background:white; }QCheckBox::indicator:checked { border:2px solid #4CAF50;image:url(%1); }QCheckBox::indicator:hover { border:2px solid #81C784; })").arg(gouPath)); // 设置复选框样式表
}
void BcbCheckBox::setCompactMode(bool compact) { setStyleSheet(compact ? m_compactStyle : styleSheet()); } // 设置紧凑模式
