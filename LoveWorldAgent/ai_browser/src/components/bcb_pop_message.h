#pragma once // 防止头文件重复包含
#include <QDialog> // 引入Qt对话框基类
#include <QLabel> // 引入Qt标签组件
class BcbButton; // 前置声明自定义按钮
class BcbPopMessage : public QDialog { // 自定义弹出消息框类
    Q_OBJECT // Qt元对象系统宏
public:
    enum IconType { Information, Warning, Critical, Question }; // 图标类型枚举
    enum ButtonType { NoButton = 0, Ok = 1, Cancel = 2, Yes = 4, No = 8 }; // 按钮类型枚举
    explicit BcbPopMessage(IconType iconType = Information, const QString& title = "", const QString& text = "", QWidget* parent = nullptr, int buttons = Ok); // 构造函数(buttons使用位或组合)
    static int information(QWidget* parent, const QString& title, const QString& text, int buttons = Ok); // 静态信息弹窗
    static int warning(QWidget* parent, const QString& title, const QString& text, int buttons = Ok); // 静态警告弹窗
    static int critical(QWidget* parent, const QString& title, const QString& text, int buttons = Ok); // 静态错误弹窗
    static int question(QWidget* parent, const QString& title, const QString& text, int buttons = Yes | No); // 静态确认弹窗
    void setTitle(const QString& title); // 设置标题
    void setText(const QString& text); // 设置内容文本
    void setIconType(IconType type); // 设置图标类型
private:
    void setupUI(); // 初始化界面
    QLabel* m_iconLabel = nullptr; // 图标标签
    QLabel* m_textLabel = nullptr; // 文本标签
    IconType m_iconType; // 图标类型
    int m_result = 0; // 返回结果
};
