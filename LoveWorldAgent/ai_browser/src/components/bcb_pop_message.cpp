#include "bcb_pop_message.h" // 引入自身头文件
#include "bcb_button.h" // 引入自定义按钮组件
#include <QVBoxLayout> // 引入Qt垂直布局
#include <QHBoxLayout> // 引入Qt水平布局
#include <QLabel> // 引入Qt标签组件
#include <QStyle> // 引入Qt样式（标准图标）
BcbPopMessage::BcbPopMessage(IconType iconType, const QString& title, const QString& text, QWidget* parent, int buttons) : QDialog(parent), m_iconType(iconType) { // 构造函数
    setWindowTitle(title); // 设置窗口标题
    m_textLabel = new QLabel(text, this); m_textLabel->setWordWrap(true); // 文本标签
    setupUI(); // 初始化界面
    if (buttons == (Yes | No)) { // 确认/取消按钮
        QHBoxLayout* btnLayout = new QHBoxLayout(); // 创建按钮布局
        btnLayout->addStretch(); // 左侧弹性空间
        BcbButton* btnOk = new BcbButton("确认", this, BcbButton::Primary); // 确认按钮
        BcbButton* btnCancel = new BcbButton("取消", this, BcbButton::Ghost); // 取消按钮
        connect(btnOk, &QPushButton::clicked, this, [this]() { m_result = 1; accept(); }); // 确认→返回1
        connect(btnCancel, &QPushButton::clicked, this, [this]() { m_result = 0; reject(); }); // 取消→返回0
        btnLayout->addWidget(btnCancel); btnLayout->addWidget(btnOk); // 取消在左、确认在右
        qobject_cast<QVBoxLayout*>(layout())->addLayout(btnLayout); // 将按钮布局加入主布局
    } else if (buttons == Ok) { // 只有确认按钮
        QHBoxLayout* btnLayout = new QHBoxLayout(); // 创建按钮布局
        btnLayout->addStretch(); // 左侧弹性空间
        BcbButton* btnOk = new BcbButton("确定", this, BcbButton::Primary); // 确定按钮
        connect(btnOk, &QPushButton::clicked, this, [this]() { m_result = 1; accept(); }); // 确定→返回1
        btnLayout->addWidget(btnOk); // 将确定按钮加入布局
        qobject_cast<QVBoxLayout*>(layout())->addLayout(btnLayout); // 将按钮布局加入主布局
    }
}
void BcbPopMessage::setupUI() { // 构建消息框UI
    QVBoxLayout* mainLayout = new QVBoxLayout(this); // 创建主垂直布局
    mainLayout->setContentsMargins(30, 25, 30, 20); mainLayout->setSpacing(15); // 设置边距和间距
    QHBoxLayout* contentLayout = new QHBoxLayout(); contentLayout->setSpacing(15); // 创建内容水平布局
    m_iconLabel = new QLabel(this); m_iconLabel->setFixedSize(26, 26); m_iconLabel->setAlignment(Qt::AlignCenter); // 图标标签(2倍大小)
    QStyle::StandardPixmap sp; // 标准图标枚举
    switch (m_iconType) { // 根据图标类型选择标准图标
    case Information: sp = QStyle::SP_MessageBoxInformation; break; // 信息图标
    case Warning: sp = QStyle::SP_MessageBoxWarning; break; // 警告图标
    case Critical: sp = QStyle::SP_MessageBoxCritical; break; // 错误图标
    case Question: sp = QStyle::SP_MessageBoxQuestion; break; // 问题图标
    default: sp = QStyle::SP_MessageBoxInformation; break; // 默认信息图标
    }
    m_iconLabel->setPixmap(style()->standardPixmap(sp).scaled(26, 26, Qt::KeepAspectRatio, Qt::SmoothTransformation)); // 2倍图标+平滑缩放
    contentLayout->addWidget(m_iconLabel); contentLayout->addWidget(m_textLabel, 1); contentLayout->addStretch(); // 图标+文本+弹性空间
    mainLayout->addLayout(contentLayout); mainLayout->addStretch(); // 内容布局+底部弹性空间
}
int BcbPopMessage::information(QWidget* parent, const QString& title, const QString& text, int buttons) { BcbPopMessage msg(Information, title, text, parent, buttons); return msg.exec() ? msg.m_result : 0; } // 静态信息弹窗
int BcbPopMessage::warning(QWidget* parent, const QString& title, const QString& text, int buttons) { BcbPopMessage msg(Warning, title, text, parent, buttons); return msg.exec() ? msg.m_result : 0; } // 静态警告弹窗
int BcbPopMessage::critical(QWidget* parent, const QString& title, const QString& text, int buttons) { BcbPopMessage msg(Critical, title, text, parent, buttons); return msg.exec() ? msg.m_result : 0; } // 静态错误弹窗
int BcbPopMessage::question(QWidget* parent, const QString& title, const QString& text, int buttons) { BcbPopMessage msg(Question, title, text, parent, buttons); return msg.exec() ? msg.m_result : 0; } // 静态确认弹窗
void BcbPopMessage::setTitle(const QString& title) { setWindowTitle(title); } // 设置标题
void BcbPopMessage::setText(const QString& text) { if (m_textLabel) m_textLabel->setText(text); } // 设置文本
void BcbPopMessage::setIconType(IconType type) { m_iconType = type; setupUI(); } // 设置图标类型
