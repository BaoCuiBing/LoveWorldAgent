# mobile_* 参考

## 坐标与元素

- `mobile_list_elements_on_screen` 返回 `{elements:[{ref,text,resource_id,class,content_desc,clickable,enabled,bounds,center}], total}`。
- `ref` 形如 `@e1`；`bounds` 为 `[x1,y1,x2,y2]`，`center` 为元素中心点。
- 点击/双击/长按/滑动支持 `ref` 或 `x/y`；截图与设备尺寸不一致时按比例换算。

## 输入与按键

- ASCII：`adb shell input text`（空格自动转 `%s`）。
- 非 ASCII：尝试 `clipper.set` 或 `ADB_INPUT_TEXT` 广播后发送 `KEYCODE_PASTE`，失败返回 `UNSUPPORTED_INPUT`。
- 按键别名：`home`、`back`、`enter`、`power`、`menu`、`recent`、`volume_up`、`volume_down`、`delete`、`tab`、`escape`；也接受 `KEYCODE_*` 或数字。

## 设备解析顺序

1. 设置中的 `adbPath`（可填 adb.exe 或 platform-tools 目录）
2. `ANDROID_HOME/platform-tools/adb.exe`
3. `ANDROID_SDK_ROOT/platform-tools/adb.exe`
4. PATH 中的 `adb`
5. `static/mobile/platform-tools/adb.exe`

## 落盘

- 截图：`static/data/mobile_screen/mobile_<yyyyMMdd_HHmmss_SSS>.png`
- 录屏：`static/data/mobile_screen/mobile_recording_<timestamp>.mp4`
- 日志：`static/logs/mobile_use.log`

## 协议

- 管道：`loveworld_mobile`
- 请求：`{"type":"req","id":N,"cmd":"mobile_xxx","args":{...}}`
- 响应：`{"type":"res","id":N,"ok":true,"data":{...}}` 或 `{"ok":false,"code":"...","error":"..."}`
- 大响应按 `chunks/chunk/data` 分片（单帧 ≤8000 字节）。