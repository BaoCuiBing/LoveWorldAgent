#pragma once // 防止头文件重复包含
#include <QWidget> // 引入Qt窗口基类
#include <QListWidget> // 引入Qt列表组件
#include <QVBoxLayout> // 引入Qt垂直布局
class BcbMenuNav : public QWidget { // 自定义侧边菜单导航类
    Q_OBJECT // Qt元对象系统宏
public:
    explicit BcbMenuNav(QWidget* parent = nullptr); // 构造函数
    void addItems(const QStringList& items); // 添加菜单项列表
    void setCurrentRow(int row); // 设置当前选中行
    int currentRow() const; // 获取当前选中行索引
signals:
    void currentRowChanged(int row); // 当前行切换信号
private:
    QListWidget* m_list; // 内部列表组件
};
