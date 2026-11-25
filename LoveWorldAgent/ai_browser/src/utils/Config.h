#pragma once // 防止头文件重复包含
#include <QString> // Qt 字符串
#include <QDir> // Qt 目录
#include <QCoreApplication> // Qt 应用（取可执行文件目录）
// 从 scanner_desktop 控件复制而来：仅实现 resolvePath（基于可执行文件目录解析 resources 资源）；原版还含配置加载，本项目只用于定位图标
class Config { // 资源路径定位工具类
public:
    static QString resolvePath(const QString& relativePath) { const QString base = QCoreApplication::applicationDirPath(); return QDir(base).absoluteFilePath(relativePath); } // 相对路径 → 基于 exe 目录的绝对路径
};
