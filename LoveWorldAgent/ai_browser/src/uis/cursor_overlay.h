#pragma once // 防止头文件重复包含
#include <QPointF> // 光标坐标
#include <QRect> // 承载区矩形
#include <QTimer> // 涟漪动画计时
#include <QWidget> // 透明覆盖层基类
class CursorOverlay : public QWidget { // 智能体模拟鼠标：独立顶层透明窗口，绘制箭头光标与点击涟漪，不拦截真实鼠标
    Q_OBJECT // Qt 元对象宏
public:
    explicit CursorOverlay(); // 构造透明置顶、鼠标穿透的覆盖层
    void SetHostGeometry(const QRect& rect); // 对齐 CEF 承载区的全局矩形
    void ShowAt(double fx, double fy, bool click); // 以 0~1 归一化坐标定位光标；click 时播放涟漪
    void HideCursor(); // 隐藏光标并停止动画
protected:
    void paintEvent(QPaintEvent* event) override; // 绘制箭头光标与涟漪
private:
    void OnTick(); // 推进涟漪动画
    QPointF pos_; // 光标热点（覆盖层内坐标）
    bool animating_ = false; // 涟漪是否播放中
    double pulse_ = 0.0; // 涟漪进度 0~1
    QTimer timer_; // 16ms 动画计时器
};