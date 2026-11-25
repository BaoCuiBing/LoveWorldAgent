#pragma once // 防止头文件重复包含
#include <QRadioButton> // 引入Qt单选按钮基类
class BcbRadioButton : public QRadioButton { // 自定义样式单选按钮类
    Q_OBJECT // Qt元对象系统宏
public:
    explicit BcbRadioButton(const QString& text, QWidget* parent = nullptr); // 构造函数
};
