#pragma once // 防止头文件重复包含
#include <QWidget> // 引入Qt窗口基类
#include <QLabel> // 引入Qt标签组件
#include <QPropertyAnimation> // 引入Qt属性动画
#include <QGraphicsDropShadowEffect> // 引入Qt图形阴影特效
class BcbSwitch : public QWidget { // 自定义开关控件类
    Q_OBJECT // Qt元对象系统宏
    Q_PROPERTY(int position READ position WRITE setPosition) // 声明属性用于动画
public:
    explicit BcbSwitch(bool checked = false, QWidget* parent = nullptr); // 构造函数
    bool isChecked() const { return m_checked; } // 获取开关状态
    void setChecked(bool checked); // 设置开关状态（带动画）
signals:
    void toggled(bool checked); // 开关切换信号（动画完成后发射）
protected:
    void mousePressEvent(QMouseEvent* event) override; // 鼠标点击事件
    void paintEvent(QPaintEvent* event) override; // 绘制事件
private:
    int position() const { return m_position; } // 获取滑块位置
    void setPosition(int pos); // 设置滑块位置并更新
    bool m_checked; // 开关状态
    int m_position = 0; // 滑块当前位置
    int m_animStart = 0, m_animEnd = 0; // 动画起止位置
};
