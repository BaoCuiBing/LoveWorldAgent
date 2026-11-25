#include "bcb_original_tab_widget.h" // 引入自身头文件
#include "utils/config.h" // 引入配置工具类
#include <QTabBar> // 引入Qt TabBar
BcbOriginalTabWidget::BcbOriginalTabWidget(QWidget* parent) : QTabWidget(parent) { // 构造函数:配置Bcb主题Tab外观
    tabBar()->setExpanding(false); // 标签不扩展
    tabBar()->setUsesScrollButtons(true); // 启用滚动按钮
    QString exitPath = Config::resolvePath("resources/icons/exit_off.png"); // 解析关闭按钮图标绝对路径
    QString closeBtnStyle = QString(R"(
        QTabBar::close-button { border-image:url(%1); subcontrol-position:right; width:16px; height:16px; margin:2px; border:none; padding:0; border-radius:4px; }
        QTabBar::close-button:hover { background:rgba(0,0,0,0.08); border-radius:4px; }
        QTabBar::close-button:pressed { background:#EF5350; border-radius:4px; }
    )").arg(exitPath); // 关闭按钮样式(VSCode风格:圆润border-radius+半透明hover+红色pressed)
    QString baseStyle = R"( // Bcb主题样式
        QTabWidget::pane { border:1px solid #E0E0E0; border-top:3px solid #4CAF50; background:white; border-radius:0; top:-1px; }
        QTabBar { background:#FAFAFA; qproperty-drawBase:0; }
        QTabBar::tab { max-width:120px; min-height:32px; padding:6px 14px; margin-right:2px; color:#757575; font-size:13px; background:transparent; border:none; border-bottom:3px solid transparent; }
        QTabBar::tab:hover:!selected { color:#424242; background:#F5F5F5; }
        QTabBar::tab:selected { color:#2E7D32; font-weight:500; background:white; border-bottom:3px solid #4CAF50; }
        QTabBar QToolButton { background:#FAFAFA; border:none; }
        QTabBar QToolButton::right-arrow:enabled { image:url(); width:0; }
        QTabBar QToolButton::left-arrow:enabled { image:url(); width:0; }
    )";
    setStyleSheet(baseStyle); // QTabWidget整体样式
    tabBar()->setStyleSheet(closeBtnStyle); // 关闭按钮样式直接设置在QTabBar上
}
QString BcbOriginalTabWidget::ellipsize(const QString& text) const { // 截断文本:超过最大长度则显示"前N字…"
    if (m_maxLength <= 0 || text.length() <= m_maxLength) return text; // 无限制或未超长直接返回
    return text.left(m_maxLength) + QChar(0x2026); // 使用Unicode省略号…(U+2026),比英文...更紧凑美观
}
int BcbOriginalTabWidget::addTab(QWidget* page, const QString& label) { // 重写addTab:对label进行截断处理
    return QTabWidget::addTab(page, ellipsize(label)); // 调用父类addTab,传入截断后的文本
}
int BcbOriginalTabWidget::insertTab(int index, QWidget* page, const QString& label) { // 重写insertTab:同样截断
    return QTabWidget::insertTab(index, page, ellipsize(label)); // 调用父类insertTab,传入截断后文本
}
void BcbOriginalTabWidget::setTabText(int index, const QString& label) { // 重写setTabText:同样截断
    QTabWidget::setTabText(index, ellipsize(label)); // 调用父类setTabText,传入截断后文本
}
void BcbOriginalTabWidget::setMaxTabTextLength(int length) { // 设置tab标签最大显示字符数
    m_maxLength = length; // 保存最大字符数
    for (int i = 0; i < count(); ++i) { // 遍历所有现有tab,重新截断显示
        QTabWidget::setTabText(i, ellipsize(QTabWidget::tabText(i))); // 重新设置截断后的文本
    }
}
int BcbOriginalTabWidget::maxTabTextLength() const { return m_maxLength; } // 获取当前最大字符数
