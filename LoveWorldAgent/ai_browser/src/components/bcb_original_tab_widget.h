#pragma once // 防止头文件重复包含
#include <QTabWidget> // 引入Qt原生TabWidget基类
class BcbOriginalTabWidget : public QTabWidget { // Bcb主题TabWidget组件(基于原生QTabWidget,适用于分类展示场景如RightElements)
    Q_OBJECT // Qt元对象系统宏
public:
    explicit BcbOriginalTabWidget(QWidget* parent = nullptr); // 构造函数
    int addTab(QWidget* page, const QString& label); // 重写addTab:超过最大字符数自动截断为"前N字…"
    int insertTab(int index, QWidget* page, const QString& label); // 重写insertTab:同样截断
    void setTabText(int index, const QString& label); // 重写setTabText:同样截断
    void setMaxTabTextLength(int length); // 设置tab标签最大显示字符数(超过则截断+省略号)
    int maxTabTextLength() const; // 获取当前最大字符数
private:
    QString ellipsize(const QString& text) const; // 截断文本辅助函数
    int m_maxLength = 5; // tab标签最大显示字符数(默认5,超过显示"前N字…")
};
