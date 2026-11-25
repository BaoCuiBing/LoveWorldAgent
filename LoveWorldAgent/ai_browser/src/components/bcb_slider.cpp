#include "bcb_slider.h" // 引入自身头文件
BcbSlider::BcbSlider(Qt::Orientation orientation, QWidget* parent) : QSlider(orientation, parent) { // 构造函数:设置滑块样式
    m_baseStyle = "QSlider::groove:horizontal{height:4px;background:#E0E0E0;border-radius:2px;}" // 滑槽:灰色4px高,圆角2px
                  "QSlider::sub-page:horizontal{background:#4CAF50;border-radius:2px;}" // 已滑过部分:绿色4px高,圆角2px
                  "QSlider::add-page:horizontal{background:#E0E0E0;border-radius:2px;}" // 未滑过部分:灰色,与groove一致
                  "QSlider::handle:horizontal{width:16px;height:16px;margin:-7px 0;border-radius:8px;background:white;border:2px solid #4CAF50;}" // 滑块:16x16正圆(margin负值居中),2px绿色边框
                  "QSlider::handle:horizontal:hover{background:#E8F5E9;border:2px solid #4CAF50;}" // 滑块hover:浅绿背景
                  "QSlider::handle:horizontal:pressed{background:#C8E6C9;border:2px solid #4CAF50;}"; // 滑块pressed:深绿背景
    setStyleSheet(m_baseStyle); // 应用默认样式
}
