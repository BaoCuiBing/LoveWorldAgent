#pragma once // 防止头文件重复包含
#include <QPushButton> // 引入Qt按钮基类
class BcbButton : public QPushButton { // 自定义样式按钮类
    Q_OBJECT // Qt元对象系统宏
public:
    enum Style { Primary, Success, Warning, Danger, Text, Ghost, Small, DangerSmall }; // 按钮样式枚举
    explicit BcbButton(const QString& text, QWidget* parent = nullptr, Style style = Primary); // 构造函数
    void setStyle(Style style); // 设置按钮样式
protected:
    void enterEvent(QEnterEvent* event) override; // 鼠标进入事件
    void leaveEvent(QEvent* event) override; // 鼠标离开事件
    void mousePressEvent(QMouseEvent* event) override; // 鼠标按下事件
    void mouseReleaseEvent(QMouseEvent* event) override; // 鼠标释放事件
private:
    Style m_style; // 当前样式
    QString m_normal, m_hover, m_pressed; // 正常、悬停、按下三种样式表
};
