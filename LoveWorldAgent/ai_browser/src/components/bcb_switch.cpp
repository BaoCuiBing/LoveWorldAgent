#include "bcb_switch.h" // 引入自身头文件
#include <QPainter> // 引入Qt绘制工具
#include <QMouseEvent> // 引入Qt鼠标事件
#include <QPainterPath> // 引入Qt绘制路径
#include <QTimer> // 引入Qt定时器(延迟发射初始化信号)
BcbSwitch::BcbSwitch(bool checked, QWidget* parent) : QWidget(parent), m_checked(checked) { // 构造函数：初始化开关状态
    setFixedSize(48, 26); // 设置固定尺寸48x26
    setCursor(Qt::PointingHandCursor); // 设置手型光标
    m_position = checked ? 22 : 0; // 根据初始状态设置滑块位置
    if (checked) QTimer::singleShot(0, this, [this]() { emit toggled(true); }); // 初始为true时延迟发射信号,确保connect()先于emit建立 (修复从DB加载触发器时无法注册UIA事件/定时任务的问题)
}
void BcbSwitch::setChecked(bool checked) { // 设置开关状态（带动画过渡）
    if (m_checked == checked) return; // 状态未变化则跳过
    m_checked = checked; // 更新状态
    m_animStart = m_position; // 记录动画起始位置
    m_animEnd = checked ? 22 : 0; // 计算动画目标位置
    QPropertyAnimation* anim = new QPropertyAnimation(this, "position"); // 创建属性动画
    anim->setDuration(150); // 动画时长150ms
    anim->setStartValue(m_animStart); // 设置起始值
    anim->setEndValue(m_animEnd); // 设置结束值
    connect(anim, &QPropertyAnimation::finished, this, [this]() { emit toggled(m_checked); }); // 动画完成后发射toggled信号
    anim->start(QAbstractAnimation::DeleteWhenStopped); // 启动动画（停止后自动删除）
}
void BcbSwitch::setPosition(int pos) { m_position = pos; update(); } // 设置滑块位置并触发重绘
void BcbSwitch::mousePressEvent(QMouseEvent* event) { if (event->button() == Qt::LeftButton) setChecked(!m_checked); } // 鼠标左键点击切换状态
void BcbSwitch::paintEvent(QPaintEvent*) { // 绘制开关控件
    QPainter p(this); // 创建绘制器
    p.setRenderHint(QPainter::Antialiasing); // 开启抗锯齿
    p.setPen(Qt::NoPen); // 无边框
    QRect bgRect(0, 3, 44, 20); // 背景槽区域
    QPainterPath path; // 绘制路径
    path.addRoundedRect(bgRect, 10, 10); // 圆角矩形路径
    p.fillPath(path, m_checked ? QColor("#4CAF50") : QColor("#E0E0E0")); // 填充背景色（开启绿色/关闭灰色）
    p.setBrush(Qt::white); // 滑块白色
    p.drawEllipse(m_position + 2, 5, 16, 16); // 绘制圆形滑块
}
