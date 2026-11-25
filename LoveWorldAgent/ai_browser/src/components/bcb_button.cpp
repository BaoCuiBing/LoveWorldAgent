#include "bcb_button.h" // 引入自身头文件
BcbButton::BcbButton(const QString& text, QWidget* parent, Style style) : QPushButton(text, parent) { setStyle(style); setCursor(Qt::PointingHandCursor); setFlat(true); } // 构造函数：设置样式、手型光标、无边框
void BcbButton::setStyle(Style style) { // 设置按钮样式
    m_style = style; // 保存样式枚举
    QString base = "font-size:13px;padding:8px 20px;border-radius:0;"; // 基础样式模板
    switch (style) { // 根据样式枚举设置对应CSS
    case Primary: m_normal = base + "background:#2196F3;color:white;border:none;"; m_hover = base + "background:#1E88E5;color:white;border:none;"; m_pressed = base + "background:#1565C0;color:white;border:none;"; break; // 主要样式（蓝色）
    case Success: m_normal = base + "background:#4CAF50;color:white;border:none;"; m_hover = base + "background:#43A047;color:white;border:none;"; m_pressed = base + "background:#2E7D32;color:white;border:none;"; break; // 成功样式（绿色）
    case Warning: m_normal = base + "background:#FF9800;color:white;border:none;"; m_hover = base + "background:#FB8C00;color:white;border:none;"; m_pressed = base + "background:#EF6C00;color:white;border:none;"; break; // 警告样式（橙色）
    case Danger: m_normal = base + "background:#F44336;color:white;border:none;"; m_hover = base + "background:#E53935;color:white;border:none;"; m_pressed = base + "background:#C62828;color:white;border:none;"; break; // 危险样式（红色）
    case Text: m_normal = "color:#1976D2;background:transparent;border:none;text-decoration:underline;font-size:13px;padding:4px 8px;"; m_hover = "color:#0D47A1;background:transparent;border:none;text-decoration:underline;font-size:13px;padding:4px 8px;"; m_pressed = m_hover; break; // 文本样式（带下划线）
    case Ghost: m_normal = base + "background:transparent;color:#4CAF50;border:1px solid #4CAF50;padding:7px 19px;"; m_hover = base + "background:#E8F5E9;color:#388E3C;border:1px solid #388E3C;padding:7px 19px;"; m_pressed = base + "background:#C8E6C9;color:#2E7D32;border:1px solid #2E7D32;padding:7px 19px;"; break; // 幽灵样式（透明背景+绿色边框，padding补偿边框）
    case Small: m_normal = "color:#1976D2;background:transparent;border:none;font-size:12px;padding:2px 6px;"; m_hover = "color:#0D47A1;background:rgba(33,150,243,0.08);border:none;font-size:12px;padding:2px 6px;"; m_pressed = m_hover; break; // 小号蓝色文字样式
    case DangerSmall: m_normal = "color:#F44336;background:transparent;border:none;font-size:12px;padding:2px 6px;"; m_hover = "color:#E53935;background:rgba(244,67,54,0.08);border:none;font-size:12px;padding:2px 6px;"; m_pressed = m_hover; break; // 小号红色文字样式
    }
    setStyleSheet(m_normal); // 应用正常状态样式
}
void BcbButton::enterEvent(QEnterEvent*) { if (!isFlat()) return; setStyleSheet(m_hover); } // 鼠标进入：切换为悬停样式（仅flat模式生效）
void BcbButton::leaveEvent(QEvent*) { if (!isFlat()) return; setStyleSheet(m_normal); } // 鼠标离开：恢复正常样式
void BcbButton::mousePressEvent(QMouseEvent* e) { if (!isFlat()) { QPushButton::mousePressEvent(e); return; } setStyleSheet(m_pressed); QPushButton::mousePressEvent(e); } // 鼠标按下：切换为按下样式
void BcbButton::mouseReleaseEvent(QMouseEvent* e) { if (!isFlat()) { QPushButton::mouseReleaseEvent(e); return; } setStyleSheet(m_hover); QPushButton::mouseReleaseEvent(e); } // 鼠标释放：恢复悬停样式
