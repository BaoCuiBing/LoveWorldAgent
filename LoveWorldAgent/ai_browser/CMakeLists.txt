# browser_panel（CEF C++ sidecar）构建配置。
#
# 前置条件（由使用者准备，本文件不负责下载）：
#   1. Visual Studio 2019 生成工具 16.11+（MSVC v142）+ Windows SDK 10.0.22621（本机已装）
#   2. CEF 111.2.7 windows64「标准版」发行包，解压到：
#      ai_browser/third_party/cef/cef_binary_111.2.7+gebf5d6a+chromium-111.0.5563.148_windows64
#      （也可用 -DCEF_ROOT=<路径> 显式指定；本工程只需 include/ 与 libcef_dll/，不需 third_party 子目录）
#
# 配置与构建（在 x64 Native Tools 环境内，或直接调用同目录 build.ps1）：
#   cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_SYSTEM_VERSION=10.0.22621
#   cmake --build build
#   产物：ai_browser/out/Release/browser_panel.exe（CEF 运行期文件自动复制到同目录）

cmake_minimum_required(VERSION 3.20)

project(browser_panel LANGUAGES CXX)

# ---------- Qt6（重写 DuiLib 外壳为 Qt Widgets） ----------
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC OFF)
find_package(Qt6 REQUIRED COMPONENTS Widgets)

if(NOT WIN32)
    message(FATAL_ERROR "browser_panel 仅支持 Windows")
endif()
if(NOT CMAKE_SIZEOF_VOID_P EQUAL 8)
    message(FATAL_ERROR "必须使用 x64 工具链（CEF windows64 发行包）")
endif()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Qt 6 与 CEF wrapper 需使用相同的 CRT；本工程最终统一为 /MT（静态 CRT），见下方 target 属性
if(MSVC)
    set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreadedDLL")
endif()

if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Release CACHE STRING "" FORCE)   # CEF 发行包默认只随 Release 提供可用二进制组合
endif()

# ---------- 定位 CEF 发行包 ----------
if(NOT CEF_ROOT)
    file(GLOB cef_candidates "${CMAKE_CURRENT_SOURCE_DIR}/third_party/cef/cef_binary_*_windows64")
    list(SORT cef_candidates)
    list(LENGTH cef_candidates cef_count)
    if(cef_count GREATER 0)
        list(GET cef_candidates -1 CEF_ROOT)
    endif()
endif()

if(NOT CEF_ROOT OR NOT EXISTS "${CEF_ROOT}/cmake")
    message(FATAL_ERROR
        "未找到 CEF 发行包。请解压 cef_binary_111.2.7..._windows64 到\n"
        "  ${CMAKE_CURRENT_SOURCE_DIR}/third_party/cef/\n"
        "或用 -DCEF_ROOT=<发行包根目录> 指定。")
endif()
message(STATUS "CEF_ROOT = ${CEF_ROOT}")

list(APPEND CMAKE_MODULE_PATH "${CEF_ROOT}/cmake")
find_package(CEF REQUIRED)    # 提供 CEF_STANDARD_LIBS / CEF_LIBCEF_DLL_WRAPPER_PATH 等

add_subdirectory(${CEF_LIBCEF_DLL_WRAPPER_PATH} libcef_dll_wrapper)

# DuiLib 已移除：UI 外壳重写为 Qt（见 src/qt 与 src/components 中的 Bcb 控件）

# ---------- 本工程 ----------
# 源码分层：src（入口 main.cpp）、src/server（管道/CEF 回调/CDP/指令分发）、src/uis（DuiLib 面板窗口、标签页、外壳菜单）、
# src/browser_tool（每个 browser_* 工具一对 cpp/h）、src/utils（JSON 与脚本构造）
file(GLOB_RECURSE browser_panel_SOURCES CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/src/*.cpp")
list(APPEND browser_panel_SOURCES
    "${CMAKE_CURRENT_SOURCE_DIR}/src/browser_tool/wait_helper.cpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/src/browser_tool/input_helper.cpp"
)
list(REMOVE_DUPLICATES browser_panel_SOURCES)
list(SORT browser_panel_SOURCES)

# 应用资源（窗口图标 dragon.ico 编进 exe）
set(browser_panel_RESOURCES "${CMAKE_CURRENT_SOURCE_DIR}/resources/browser_panel.rc")

add_executable(browser_panel WIN32 ${browser_panel_SOURCES} ${browser_panel_RESOURCES})

# 链接为静态 CRT（__imp_ 未解析即静态链接的标志）：显式将 browser_panel 也锁为 /MT，与 duilib/CEF wrapper 对齐
set_target_properties(browser_panel PROPERTIES MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")

set_target_properties(browser_panel PROPERTIES
    PROJECT_LABEL "browser_panel"
    RUNTIME_OUTPUT_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/out/$<CONFIG>"
)

target_include_directories(browser_panel PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/src"
    "${CMAKE_CURRENT_SOURCE_DIR}/resources"
    "${CEF_ROOT}"
)

target_compile_definitions(browser_panel PRIVATE
    UNICODE _UNICODE NOMINMAX WIN32_LEAN_AND_MEAN _CRT_SECURE_NO_WARNINGS
)

if(MSVC)
    # CEF 会为本可执行目标开启 /WX；引入 DuiLib 后其头文件存在大量无害告警，故关闭“告警即错误”并放宽常见告警
    target_compile_options(browser_panel PRIVATE /utf-8 /W3 /MP /WX- /wd4091 /wd4010 /wd4100 /wd4005 /wd4189 /wd4267 /wd4244 /wd4819)
endif()

# Windows 下 CEF_STANDARD_LIBS 不含 libcef 导入库，需按 CEF 官方宏建逻辑目标后链接
if(NOT DEFINED CEF_LIB_RELEASE)
    message(FATAL_ERROR "find_package(CEF) 未提供 CEF_LIB_RELEASE，请确认 CEF_ROOT 指向完整发行包")
endif()
ADD_LOGICAL_TARGET(libcef_impl "${CEF_LIB_DEBUG}" "${CEF_LIB_RELEASE}")

target_link_libraries(browser_panel PRIVATE
    libcef_dll_wrapper
    libcef_impl
    Qt6::Widgets
    ${CEF_STANDARD_LIBS}
    user32 gdi32 shell32 ole32 oleaut32 uuid comctl32 gdiplus imm32 winmm uxtheme advapi32 propsys
)
# WIN32 子系统 + main() 入口：覆盖默认 WinMain 入口，避免弹出控制台窗口
target_link_options(browser_panel PRIVATE "/ENTRY:mainCRTStartup")

# CEF 要求的编译/链接选项（含 /STACK 调整）
SET_EXECUTABLE_TARGET_PROPERTIES(browser_panel)
# CEF 强制 /WX（告警即错误），生成器表达式会被放到命令行末尾，确保 /WX- 盖掉它
target_compile_options(browser_panel PRIVATE
    "$<$<CONFIG:Release>:/WX->"
    "$<$<CONFIG:Release>:/wd4458>"
    "$<$<CONFIG:Release>:/wd4459>")

# ---------- 把 CEF 运行期文件（dll 与 pak/locales）平铺到 exe 同级 ----------
if(CMAKE_BUILD_TYPE STREQUAL "Debug" AND EXISTS "${CEF_BINARY_DIR_DEBUG}")
    set(cef_active_binary_dir "${CEF_BINARY_DIR_DEBUG}")
else()
    set(cef_active_binary_dir "${CEF_BINARY_DIR_RELEASE}")
endif()
COPY_FILES(browser_panel "${CEF_BINARY_FILES}" "${cef_active_binary_dir}" "$<TARGET_FILE_DIR:browser_panel>")
COPY_RESOURCES(browser_panel "${CEF_RESOURCE_FILES}" "" "${CEF_RESOURCE_DIR}" "$<TARGET_FILE_DIR:browser_panel>")

PRINT_CEF_CONFIG()

# ---------- Qt 部署与静态资源拷贝 ----------
find_program(WINDEPLOYQT_EXECUTABLE windeployqt HINTS "${Qt6_DIR}/../../../bin")
if(WINDEPLOYQT_EXECUTABLE)
    add_custom_command(TARGET browser_panel POST_BUILD
        COMMAND "${WINDEPLOYQT_EXECUTABLE}" --release --no-translations --no-opengl-sw --no-system-d3d-compiler "$<TARGET_FILE:browser_panel>"
        COMMENT "Deploying Qt runtime libraries")
endif()
# 运行期资源：resources/（browser_panel.qss 与 icons/*.png，控件经 Config::resolvePath 按 exe 同级目录读取）
add_custom_command(TARGET browser_panel POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_directory "${CMAKE_CURRENT_SOURCE_DIR}/resources" "$<TARGET_FILE_DIR:browser_panel>/resources")
