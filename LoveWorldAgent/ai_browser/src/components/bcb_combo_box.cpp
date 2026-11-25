#include "bcb_combo_box.h" // 引入自身头文件
#include "utils/config.h" // 引入配置工具类
#include <QAbstractItemView> // 引入Qt抽象列表视图
#include <QGraphicsDropShadowEffect> // 引入Qt图形阴影特效
BcbComboBox::BcbComboBox(QWidget* parent) : QComboBox(parent) { // 构造函数：构建自定义样式下拉框
    QString downPath = Config::resolvePath("resources/icons/jiantou_down.png"); // 解析向下箭头图标路径
    QString upPath = Config::resolvePath("resources/icons/jiantou_up.png"); // 解析向上箭头图标路径
    // 下拉列表弹出区域样式
    QString itemArea = R"(
        QComboBox QAbstractItemView { border:1px solid #E0E0E0;border-radius:0;padding:4px;outline:none;font-size:13px;background:#FAFAFA;selection-background-color:#C8E6C9;selection-color:#1B5E20; }
        QComboBox QAbstractItemView::item { padding:3px 12px;min-height:22px;border-radius:0;margin:1px 2px; }
        QComboBox QAbstractItemView::item:hover { background-color:#E8F5E9;color:#2E7D32; }
        QComboBox QAbstractItemView::item:selected { background-color:#A5D6A7;color:#1B5E20; }
        QComboBox QAbstractItemView QScrollBar:vertical { width:6px;background:transparent;margin:0; }
        QComboBox QAbstractItemView QScrollBar::handle:vertical { background:#BDBDBD;border-radius:0;min-height:20px; }
        QComboBox QAbstractItemView QScrollBar::handle:vertical:hover { background:#9E9E9E; }
        QComboBox QAbstractItemView QScrollBar::add-line:vertical, QComboBox QAbstractItemView QScrollBar::sub-line:vertical { height:0; }
    )";
    // 默认样式（未聚焦）
    m_baseStyle = QString(R"(
        QComboBox { padding:6px 28px 6px 12px;border:1px solid #E0E0E0;border-radius:0;font-size:13px;background:white; }
        QComboBox:focus { padding:5px 27px 5px 11px;border:2px solid #4CAF50;border-radius:0; }
        QComboBox::drop-down { subcontrol-origin:padding;subcontrol-position:top right;width:24px;border:none;background:transparent; }
        QComboBox::down-arrow { image:url(%1);width:20px;height:12px; }
        QComboBox::down-arrow:on { image:url(%2);width:20px;height:12px; }
        %3
    )").arg(downPath, upPath, itemArea); // 替换箭头图标和列表样式
    // 焦点样式
    m_focusStyle = QString(R"(
        QComboBox { padding:5px 27px 5px 11px;border:2px solid #4CAF50;border-radius:0;font-size:13px;background:white; }
        QComboBox::drop-down { subcontrol-origin:padding;subcontrol-position:top right;width:24px;border:none;background:transparent; }
        QComboBox::down-arrow { image:url(%1);width:20px;height:12px; }
        QComboBox::down-arrow:on { image:url(%2);width:20px;height:12px; }
        %3
    )").arg(downPath, upPath, itemArea); // 拼接焦点样式表
    // 紧凑模式默认样式(适配表格单元格)
    m_compactBaseStyle = QString(R"(
        QComboBox { margin:0;padding:3px 20px 3px 4px;border:1px solid #E0E0E0;border-radius:0;font-size:12px;background:white; }
        QComboBox:focus { margin:0;padding:3px 20px 3px 4px;border:1px solid #4CAF50;border-radius:0; }
        QComboBox::drop-down { subcontrol-origin:padding;subcontrol-position:top right;width:18px;border:none;background:transparent; }
        QComboBox::down-arrow { image:url(%1);width:14px;height:8px; }
        QComboBox::down-arrow:on { image:url(%2);width:14px;height:8px; }
        %3
    )").arg(downPath, upPath, itemArea); // 拼接紧凑默认样式表
    // 紧凑模式焦点样式
    m_compactFocusStyle = QString(R"(
        QComboBox { margin:0;padding:3px 20px 3px 4px;border:1px solid #4CAF50;border-radius:0;font-size:12px;background:white; }
        QComboBox::drop-down { subcontrol-origin:padding;subcontrol-position:top right;width:18px;border:none;background:transparent; }
        QComboBox::down-arrow { image:url(%1);width:14px;height:8px; }
        QComboBox::down-arrow:on { image:url(%2);width:14px;height:8px; }
        %3
    )").arg(downPath, upPath, itemArea); // 拼接紧凑焦点样式表
    setStyleSheet(m_baseStyle); // 应用默认样式
    if (auto* itemView = view()) { // 为下拉列表添加阴影效果
        itemView->setVerticalScrollMode(QAbstractItemView::ScrollPerPixel); // 设置逐像素滚动
        itemView->window()->setWindowFlags(Qt::Popup | Qt::FramelessWindowHint); // 弹出窗口无边框
        itemView->window()->setAttribute(Qt::WA_TranslucentBackground); // 透明背景
        auto* shadow = new QGraphicsDropShadowEffect(itemView->window()); // 创建阴影特效
        shadow->setBlurRadius(12); // 阴影模糊半径
        shadow->setColor(QColor(0, 0, 0, 50)); // 阴影颜色
        shadow->setOffset(0, 2); // 阴影偏移
        itemView->window()->setGraphicsEffect(shadow); // 应用阴影
        itemView->setMinimumWidth(120); // 下拉列表最小宽度(不受控件总宽度约束)
    }
    setMaxVisibleItems(12); // 最多显示12项
}
void BcbComboBox::setCompactMode(bool compact) { m_compact = compact; if (compact) setStyleSheet(m_compactBaseStyle); else setStyleSheet(m_baseStyle); } // 设置紧凑模式
void BcbComboBox::focusInEvent(QFocusEvent* e) { setStyleSheet(m_compact ? m_compactFocusStyle : m_focusStyle); QComboBox::focusInEvent(e); } // 获得焦点：切换为焦点样式
void BcbComboBox::focusOutEvent(QFocusEvent* e) { setStyleSheet(m_compact ? m_compactBaseStyle : m_baseStyle); QComboBox::focusOutEvent(e); } // 失去焦点：恢复默认样式
