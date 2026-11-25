#pragma once // 防止头文件重复包含
#include < QLineEdit> // 引入Qt单行输入框基类
class BcbLineEdit : public QLineEdit { // 自定义样式输入框类
    Q_OBJECT // Qt元对象系统宏
public:
    explicit BcbLineEdit(QWidget* parent = nullptr); // 构造函数
    void setCompactMode(bool compact); // 设置紧凑模式(适配表格单元格)
private:
    QString m_compactBaseStyle; // 紧凑模式默认样式
    QString m_compactFocusStyle; // 紧凑模式焦点样式
    bool m_compact = false; // 是否为紧凑模式
protected:
    void focusInEvent(QFocusEvent* event) override; // 获得焦点事件
    void focusOutEvent(QFocusEvent* event) override; // 失去焦点事件
};
