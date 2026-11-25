---
alwaysApply: true
---
1. Python 代码格式参考项目已有 py 文件：紧凑、双引号、模块级 docstring、函数一行 docstring。
2. Vue / JS / CSS / HTML 参考已有文件风格：2 空格或 4 空格与所处文件保持一致，不引入新格式化工具。
3. 任何路径、端口、密钥类事实只从 `src-tauri/static/config.json` 与 `config.py` 读取，不在业务代码里硬编码。
4. 日志统一走 `backend.utils.logger.get_logger`，禁止 `print`（脚本与临时验证除外）。
5. 新增 Python 模块必须同步：`build/LoveWorldAgent.spec` 的 hiddenimports、`mds/projects_mds/` 对应文档。
6. 删除或结束进程前必须确认目标属于本项目，禁止批量 kill 全部 node/python 进程。
