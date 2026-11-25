#include "bcb_tree_widget.h" // 引入自身头文件
BcbTreeWidget::BcbTreeWidget(QWidget* parent) : QTreeWidget(parent) { // 构造函数:设置Bcb主题树外观
    setUniformRowHeights(true); // 统一行高(提升渲染性能)
    setIndentation(16); // 缩进16px
    setVerticalScrollMode(QAbstractItemView::ScrollPerPixel); // 逐像素垂直滚动
    setHorizontalScrollMode(QAbstractItemView::ScrollPerPixel); // 逐像素水平滚动
    setStyleSheet(R"(
        QTreeWidget { border:none; background:white; outline:0; font-size:13px; }
        QTreeWidget::item { height:28px; padding:0 8px; border:0px; }
        QTreeWidget::item:hover:!selected { background:#F5F5F5; }
        QTreeWidget::item:selected { background:#E8F5E9; color:#2E7D32; border:0px; }
        QTreeWidget QScrollBar:vertical { width:6px; background:transparent; margin:0; }
        QTreeWidget QScrollBar::handle:vertical { background:#C0C0C0; border-radius:0; min-height:30px; }
        QTreeWidget QScrollBar::handle:vertical:hover { background:#A0A0A0; }
        QTreeWidget QScrollBar::add-line:vertical, QTreeWidget QScrollBar::sub-line:vertical { height:0; }
        QTreeWidget QScrollBar:horizontal { height:6px; background:transparent; margin:0; }
        QTreeWidget QScrollBar::handle:horizontal { background:#C0C0C0; border-radius:0; min-width:30px; }
        QTreeWidget QScrollBar::handle:horizontal:hover { background:#A0A0A0; }
        QTreeWidget QScrollBar::add-line:horizontal, QTreeWidget QScrollBar::sub-line:horizontal { width:0; }
    )");
}
