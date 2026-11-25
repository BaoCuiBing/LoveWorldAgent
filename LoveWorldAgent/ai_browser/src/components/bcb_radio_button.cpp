#include "bcb_radio_button.h" // 引入自身头文件
#include "utils/config.h" // 引入配置工具类（用于资源路径解析）
BcbRadioButton::BcbRadioButton(const QString& text, QWidget* parent) : QRadioButton(text, parent) { // 构造函数：设置圆形选中图标样式
    QString yuanPath = Config::resolvePath("resources/icons/green_yuan.png"); // 解析绿色圆点图标路径
    setStyleSheet(QString(R"(QRadioButton { font-size:13px;color:#424242;spacing:8px; }QRadioButton::indicator { width:16px;height:16px;border-radius:10px;border:2px solid #E0E0E0;background:white; }QRadioButton::indicator:checked { border:2px solid #4CAF50;image:url(%1); })").arg(yuanPath)); // 设置单选按钮样式表
}
