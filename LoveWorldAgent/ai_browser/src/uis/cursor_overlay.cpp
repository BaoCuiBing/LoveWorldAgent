#include "uis/cursor_overlay.h" // 自身声明
#include <QColor> // 光标与涟漪配色
#include <QPainter> // 绘制
#include <QPen> // 描边
#include <QPolygonF> // 箭头多边形
CursorOverlay::CursorOverlay() : QWidget(nullptr) { // 独立顶层窗口：置顶、无边框、透明、输入穿透
    setWindowFlags(Qt::Tool | Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint | Qt::WindowTransparentForInput | Qt::NoDropShadowWindowHint);
    setAttribute(Qt::WA_TranslucentBackground); // 背景透明
    setAttribute(Qt::WA_ShowWithoutActivating); // 显示时不抢焦点
    setAttribute(Qt::WA_TransparentForMouseEvents); // 不拦截鼠标
    setAttribute(Qt::WA_QuitOnClose, false); // 不阻止主窗口退出时结束事件循环
    setFocusPolicy(Qt::NoFocus); // 不参与键盘焦点
    hide(); // 初始隐藏，首次调用 ShowAt 时显示
    timer_.setInterval(16); // 约 60fps
    connect(&timer_, &QTimer::timeout, this, &CursorOverlay::OnTick); // 计时推进涟漪
}
void CursorOverlay::SetHostGeometry(const QRect& rect) { setGeometry(rect); update(); } // 对齐承载区并重绘
void CursorOverlay::ShowAt(double fx, double fy, bool click) { // 归一化坐标定位；click 播放涟漪
    pos_.setX(qBound(0.0, fx, 1.0) * width()); // 横向像素
    pos_.setY(qBound(0.0, fy, 1.0) * height()); // 纵向像素
    if (!isVisible()) show(); // 首次显示顶层窗口
    if (click) { pulse_ = 0.0; animating_ = true; if (!timer_.isActive()) timer_.start(); } // 重置并启动涟漪
    update(); // 立即重绘
}
void CursorOverlay::HideCursor() { animating_ = false; timer_.stop(); hide(); } // 停止动画并隐藏
void CursorOverlay::OnTick() { // 涟漪推进到 1 后停止
    if (!animating_) { timer_.stop(); return; }
    pulse_ += 0.06; // 每帧约 6%
    if (pulse_ >= 1.0) { pulse_ = 1.0; animating_ = false; timer_.stop(); }
    update();
}
void CursorOverlay::paintEvent(QPaintEvent* event) { // 先画涟漪，再画箭头光标
    Q_UNUSED(event); // 未使用事件
    QPainter p(this); // 绘制器
    p.setRenderHint(QPainter::Antialiasing, true); // 抗锯齿
    const QPointF tip = pos_; // 热点即箭头尖端
    if (pulse_ > 0.0 && pulse_ < 1.0) { // 涟漪：半径扩大、透明度衰减
        const qreal r = 6.0 + pulse_ * 26.0; // 扩散半径
        const int alpha = static_cast<int>((1.0 - pulse_) * 150); // 逐渐透明
        p.setPen(Qt::NoPen); // 无描边
        p.setBrush(QColor(70, 140, 255, alpha)); // 蓝色涟漪
        p.drawEllipse(tip, r, r); // 圆形扩散
    }
    QPolygonF arrow; // 经典箭头轮廓（尖端在热点）
    arrow << tip
          << QPointF(tip.x() + 12, tip.y() + 4)
          << QPointF(tip.x() + 5, tip.y() + 6)
          << QPointF(tip.x() + 8, tip.y() + 14)
          << QPointF(tip.x() + 4, tip.y() + 16)
          << QPointF(tip.x() + 2, tip.y() + 8)
          << QPointF(tip.x(), tip.y() + 13);
    p.setPen(QPen(QColor(255, 255, 255, 230), 1.4)); // 白色描边提升对比
    p.setBrush(QColor(20, 20, 20, 220)); // 深色填充
    p.drawPolygon(arrow); // 绘制箭头
}