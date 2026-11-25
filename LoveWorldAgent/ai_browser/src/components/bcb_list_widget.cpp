#include "bcb_list_widget.h" // 引入自身头文件
#include <QFont> // 引入QFont(分组标题加粗)
static const QString INDENT_PREFIX = QStringLiteral("　　"); // 节点/数据项左侧缩进:2个全角空格(约等于4个半角字符宽度)
BcbListWidget::BcbListWidget(QWidget* parent) : QListWidget(parent) { // 构造函数:设置Bcb主题列表外观
    setVerticalScrollMode(QAbstractItemView::ScrollPerPixel); // 逐像素垂直滚动
    setHorizontalScrollMode(QAbstractItemView::ScrollPerPixel); // 逐像素水平滚动
    applyStyle(); // 应用样式
}
void BcbListWidget::setItemVerticalPadding(int padding) { // 设置item上下内边距(不影响28px固定高度,仅作保留)
    m_padding = padding; // 保存内边距值
    applyStyle(); // 重新应用样式(滚动条等)
}
int BcbListWidget::itemVerticalPadding() const { return m_padding; } // 获取item上下内边距
QListWidgetItem* BcbListWidget::addSectionItem(const QString& title) { // 添加分组标题item(C++端直接设置加粗+灰背景+深灰字,固定高度34px)
    auto* item = new QListWidgetItem(title, this); // 创建分组标题项
    item->setFlags(Qt::ItemIsEnabled); // 仅启用,不可选中不可拖拽(作为分组标题)
    QFont font = item->font(); font.setBold(true); item->setFont(font); // 加粗显示
    item->setBackground(QColor("#F5F5F5")); // 浅灰背景区分
    item->setForeground(QColor("#616161")); // 深灰文字
    item->setSizeHint(QSize(item->sizeHint().width(), 34)); // 强制固定高度34px
    return item; // 返回标题项
}
QListWidgetItem* BcbListWidget::addDataItem(const QString& text, const QVariant& userData) { // 添加可拖拽数据item(带左侧2全角空格缩进,固定高度34px)
    auto* item = new QListWidgetItem(INDENT_PREFIX + text, this); // 文本前加缩进前缀
    item->setFlags(Qt::ItemIsEnabled | Qt::ItemIsSelectable | Qt::ItemIsDragEnabled); // 启用+可选+可拖拽
    if (userData.isValid()) item->setData(Qt::UserRole, userData); // 存储用户数据
    item->setSizeHint(QSize(item->sizeHint().width(), 34)); // 强制固定高度34px
    return item; // 返回数据项
}
void BcbListWidget::setDarkTheme(bool dark) { // 设置暗色主题并刷新样式
    m_darkTheme = dark; applyStyle(); // 保存主题状态并刷新
}
bool BcbListWidget::isDarkTheme() const { return m_darkTheme; } // 获取暗色主题状态
void BcbListWidget::setSelectionBorderVisible(bool visible) { // 设置选中边框可见性并刷新样式
    m_selectionBorderVisible = visible; applyStyle(); // 保存边框状态并刷新
}
bool BcbListWidget::isSelectionBorderVisible() const { return m_selectionBorderVisible; } // 获取边框可见性
void BcbListWidget::applyStyle() { // 应用Bcb主题样式表(根据m_darkTheme/m_selectionBorderVisible动态生成)
    QString bg = m_darkTheme ? QStringLiteral("#2b2b2b") : QStringLiteral("white"); // 背景色
    QString textColor = m_darkTheme ? QStringLiteral("#d4d4d4") : QStringLiteral("black"); // 文字色
    QString hoverBg = m_darkTheme ? QStringLiteral("#3c3c3f") : QStringLiteral("#F5F5F5"); // 悬停背景
    QString selBg = m_darkTheme ? QStringLiteral("#264f78") : QStringLiteral("#E8F5E9"); // 选中背景
    QString selColor = m_darkTheme ? QStringLiteral("#ffffff") : QStringLiteral("#2E7D32"); // 选中文字色
    QString selBorder = m_selectionBorderVisible ? QStringLiteral("border-left:3px solid #4CAF50;") : QString(); // 选中左侧绿边框
    QString scrollHandle = m_darkTheme ? QStringLiteral("#555555") : QStringLiteral("#C0C0C0"); // 滚动条手柄
    QString scrollHandleHover = m_darkTheme ? QStringLiteral("#777777") : QStringLiteral("#A0A0A0"); // 滚动条手柄悬停
    setStyleSheet(QStringLiteral( // 逐段拼接整体样式表
        "QListWidget { border:none; background:%1; outline:0; font-size:13px; color:%2; }" // 列表整体:无边框+背景+字色
        "QListWidget::item { border:0px solid transparent; padding:4px 8px; }" // 列表项:透明边框+内边距
        "QListWidget::item:hover:!selected { background:%3; }" // 悬停且未选中的背景
        "QListWidget::item:selected { background:%4; color:%5; %6 }" // 选中背景+字色+左边框
        "QListWidget QScrollBar:vertical { width:6px; background:transparent; margin:0; }" // 垂直滚动条槽透明
        "QListWidget QScrollBar::handle:vertical { background:%7; border-radius:0; min-height:30px; }" // 垂直滚动条手柄
        "QListWidget QScrollBar::handle:vertical:hover { background:%8; }" // 垂直手柄悬停色
        "QListWidget QScrollBar::add-line:vertical, QListWidget QScrollBar::sub-line:vertical { height:0; }" // 隐藏垂直增减箭头
        "QListWidget QScrollBar:horizontal { height:6px; background:transparent; margin:0; }" // 水平滚动条槽透明
        "QListWidget QScrollBar::handle:horizontal { background:%7; border-radius:0; min-width:30px; }" // 水平滚动条手柄
        "QListWidget QScrollBar::handle:horizontal:hover { background:%8; }" // 水平手柄悬停色
        "QListWidget QScrollBar::add-line:horizontal, QListWidget QScrollBar::sub-line:horizontal { width:0; }" // 隐藏水平增减箭头
    ).arg(bg, textColor, hoverBg, selBg, selColor, selBorder, scrollHandle, scrollHandleHover)); // 注入颜色参数生成最终样式
}
