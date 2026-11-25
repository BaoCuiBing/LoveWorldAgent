#pragma once // 防止头文件重复包含
#include <QListWidget> // 引入Qt列表基类
class BcbListWidget : public QListWidget { // Bcb主题列表组件(基于原生QListWidget,默认34px固定行高+节点项左侧缩进,适用于节点/流程列表展示)
    Q_OBJECT // Qt元对象系统宏
public:
    explicit BcbListWidget(QWidget* parent = nullptr); // 构造函数
    void setItemVerticalPadding(int padding); // 设置item的上下内边距(默认8px,不影响固定高度28px)
    int itemVerticalPadding() const; // 获取item上下内边距
    void setDarkTheme(bool dark); // 设置暗色主题(默认false浅色)
    bool isDarkTheme() const; // 获取暗色主题状态
    void setSelectionBorderVisible(bool visible); // 设置选中项左侧绿色边框可见性(默认true可见)
    bool isSelectionBorderVisible() const; // 获取边框可见性
    QListWidgetItem* addSectionItem(const QString& title); // 添加分组标题item(不可选中,加粗+灰背景+深灰字)
    QListWidgetItem* addDataItem(const QString& text, const QVariant& userData = QVariant()); // 添加可拖拽数据item(带左侧缩进)
private:
    void applyStyle(); // 应用Bcb主题样式表
    int m_padding = 8; // item上下内边距(像素,默认8px,不影响28px固定高度)
    bool m_darkTheme = false; // 是否暗色主题
    bool m_selectionBorderVisible = true; // 选中项左侧绿色边框是否可见
};
