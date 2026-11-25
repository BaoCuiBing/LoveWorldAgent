#include "uis/qt_theme.h" // 自身声明
#include <QApplication> // qApp
#include <QCoreApplication> // applicationDirPath
#include <QDir> // 路径拼接
#include <QFile> // 读文件
#include <QTextStream> // 流读取
namespace QtTheme {
void Apply() { const QString path = QDir(QCoreApplication::applicationDirPath()).filePath("resources/browser_panel.qss"); QFile file(path); if (file.open(QIODevice::ReadOnly | QIODevice::Text)) { QTextStream stream(&file); stream.setEncoding(QStringConverter::Utf8); const QString sheet = stream.readAll(); if (!sheet.isEmpty()) qApp->setStyleSheet(sheet); file.close(); } } // 从 exe 目录读 QSS 并应用全局样式；缺失则静默保持默认
}
