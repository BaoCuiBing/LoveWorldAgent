#pragma once // 防止头文件重复包含
#include <QTreeWidget> // 引入Qt原生TreeWidget基类
class BcbTreeWidget : public QTreeWidget { // Bcb主题树组件(基于原生QTreeWidget,28px固定行高+绿色选中左边框,适用于树形结构展示)
    Q_OBJECT // Qt元对象系统宏
public:
    explicit BcbTreeWidget(QWidget* parent = nullptr); // 构造函数
};
