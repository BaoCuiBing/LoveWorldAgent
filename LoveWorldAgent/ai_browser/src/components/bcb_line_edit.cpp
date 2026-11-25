#include "bcb_line_edit.h" // 引入自身头文件
BcbLineEdit::BcbLineEdit(QWidget* parent) : QLineEdit(parent) { // 构造函数：初始化样式
    m_compactBaseStyle = "margin:0;padding:3px 4px;border:1px solid #E0E0E0;border-radius:0;font-size:11px;"; // 紧凑模式默认样式
    m_compactFocusStyle = "margin:0;padding:3px 4px;border:1px solid #4CAF50;border-radius:0;font-size:11px;"; // 紧凑模式焦点样式
    setStyleSheet("padding:6px 12px;border:1px solid #E0E0E0;border-radius:0;font-size:13px;"); // 应用默认样式
} // 构造函数：设置默认样式和紧凑样式
void BcbLineEdit::setCompactMode(bool compact) { // 设置紧凑模式开关
    m_compact = compact; // 保存紧凑模式标志
    if (compact) { // 紧凑模式分支
        setStyleSheet(hasFocus() ? m_compactFocusStyle : m_compactBaseStyle); // 按焦点状态选样式
    } else { // 普通模式分支
        setStyleSheet(hasFocus() ? "padding:5px 11px;border:2px solid #4CAF50;border-radius:0;font-size:13px;" : "padding:6px 12px;border:1px solid #E0E0E0;border-radius:0;font-size:13px;"); // 按焦点状态选普通样式
    }
} // 设置紧凑模式
void BcbLineEdit::focusInEvent(QFocusEvent* e) { // 重写获得焦点事件
    if (m_compact) { // 紧凑模式
        setStyleSheet(m_compactFocusStyle); // 应用紧凑焦点样式
    } else { // 普通模式
        setStyleSheet("padding:5px 11px;border:2px solid #4CAF50;border-radius:0;font-size:13px;"); // 应用普通焦点样式
    }
    QLineEdit::focusInEvent(e); // 传递给基类处理
} // 获得焦点：切换为绿色边框样式
void BcbLineEdit::focusOutEvent(QFocusEvent* e) { // 重写失去焦点事件
    if (m_compact) { // 紧凑模式
        setStyleSheet(m_compactBaseStyle); // 应用紧凑默认样式
    } else { // 普通模式
        setStyleSheet("padding:6px 12px;border:1px solid #E0E0E0;border-radius:0;font-size:13px;"); // 应用普通默认样式
    }
    QLineEdit::focusOutEvent(e); // 传递给基类处理
} // 失去焦点：恢复默认灰色边框样式
