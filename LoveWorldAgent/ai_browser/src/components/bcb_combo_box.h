#pragma once // 防止头文件重复包含
#include <QComboBox> // 引入Qt下拉框基类
class BcbComboBox : public QComboBox { // 自定义样式下拉框类
    Q_OBJECT // Qt元对象系统宏
public:
    explicit BcbComboBox(QWidget* parent = nullptr); // 构造函数
    void setCompactMode(bool compact); // 设置紧凑模式(适配表格单元格)
protected:
    void focusInEvent(QFocusEvent* event) override; // 获得焦点事件
    void focusOutEvent(QFocusEvent* event) override; // 失去焦点事件
private:
    QString m_baseStyle; // 默认样式表
    QString m_focusStyle; // 焦点样式表
    QString m_compactBaseStyle; // 紧凑模式默认样式
    QString m_compactFocusStyle; // 紧凑模式焦点样式
    bool m_compact = false; // 是否为紧凑模式
};
