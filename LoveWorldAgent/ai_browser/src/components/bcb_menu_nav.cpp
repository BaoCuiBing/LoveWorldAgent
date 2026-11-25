#include "bcb_menu_nav.h" // 引入自身头文件
BcbMenuNav::BcbMenuNav(QWidget* parent) : QWidget(parent) { // 构造函数：创建侧边菜单导航
    QVBoxLayout* layout = new QVBoxLayout(this); // 创建垂直布局
    layout->setContentsMargins(0, 0, 0, 0); // 设置零边距
    layout->setSpacing(0); // 设置零间距
    m_list = new QListWidget(this); // 创建列表组件
    m_list->setFixedWidth(180); // 设置列表固定宽度
    // 设置列表样式(左侧边框高亮选中项)
    m_list->setStyleSheet(R"(
        QListWidget { border:none;background:#FAFAFA;font-size:13px;outline:none; }
        QListWidget::item { padding:10px 16px 10px 16px; border:none;border-left:3px solid transparent;color:#424242; }
        QListWidget::item:selected { background:#E8F5E9;color:#2E7D32; border-left:3px solid #4CAF50; }
        QListWidget::item:hover:!selected { background:#F5F5F5; }
    )");
    layout->addWidget(m_list); // 将列表加入布局
    connect(m_list, &QListWidget::currentRowChanged, this, &BcbMenuNav::currentRowChanged); // 转发当前行变化信号
}
void BcbMenuNav::addItems(const QStringList& items) { m_list->addItems(items); } // 添加多个菜单项
void BcbMenuNav::setCurrentRow(int row) { m_list->setCurrentRow(row); } // 设置当前选中行
int BcbMenuNav::currentRow() const { return m_list->currentRow(); } // 获取当前选中行索引
