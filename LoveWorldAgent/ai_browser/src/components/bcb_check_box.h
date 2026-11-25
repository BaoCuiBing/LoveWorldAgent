#pragma once // 防止头文件重复包含
#include <QCheckBox> // 引入Qt复选框基类
class BcbCheckBox : public QCheckBox { // 自定义样式复选框类
    Q_OBJECT // Qt元对象系统宏
public:
    explicit BcbCheckBox(QWidget* parent = nullptr); // 默认构造函数
    explicit BcbCheckBox(const QString& text, QWidget* parent = nullptr); // 构造函数
    void setCompactMode(bool compact); // 设置紧凑模式(适配表格单元格)
private:
    QString m_compactStyle; // 紧凑模式样式
};
