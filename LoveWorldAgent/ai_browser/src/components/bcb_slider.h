#pragma once // 防止头文件重复包含
#include <QSlider> // 引入Qt滑块基类
class BcbSlider : public QSlider { // Bcb主题滑块类
    Q_OBJECT // Qt元对象系统宏
public:
    explicit BcbSlider(Qt::Orientation orientation, QWidget* parent = nullptr); // 构造函数(水平/垂直方向)
private:
    QString m_baseStyle; // 默认样式
};
