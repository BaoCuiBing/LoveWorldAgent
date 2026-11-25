<template>
    <div class="tool-card kind-builtin" :class="{ open }">
        <div class="tool-head" role="button" tabindex="0" :aria-expanded="open" @click="toggle" @keydown.enter.prevent="toggle" @keydown.space.prevent="toggle">
            <component :is="iconOf" class="tool-icon" :size="14" />
            <span class="tool-title">{{ title }}</span>
            <span v-if="statusText" class="tool-status" :class="status">
                <Clock3 v-if="status === 'running'" :size="14" />
                <CircleX v-else-if="status === 'failed'" :size="14" />
                <TriangleAlert v-else-if="status === 'pending'" :size="14" />
                <CircleCheck v-else :size="14" />
                {{ statusText }}
            </span>
            <span v-if="summary" class="tool-summary">{{ summary }}</span>
            <span v-if="durationText" class="tool-duration">{{ durationText }}</span>
            <ChevronRight class="arrow" :class="{ open }" :size="12" />
        </div>
        <div v-if="open" class="tool-body">
            <div class="tool-section">
                <div class="tool-label">调用参数</div>
                <CodeEditor :model-value="argsText" language="json" read-only :height="codeHeight(argsText)" />
            </div>
            <div class="tool-section">
                <div class="tool-label" :class="{ bad: status === 'failed' }">返回结果</div>
                <CodeEditor :model-value="outputText" :language="resultLang" read-only :height="codeHeight(outputText)" />
            </div>
        </div>
    </div>
</template>

<script setup>
import { computed, ref } from 'vue'
import { ChevronRight, CircleCheck, CircleX, Clock3, FileText, Globe, Monitor, Presentation, Sheet, Smartphone, TriangleAlert, Wrench } from 'lucide-vue-next'
import CodeEditor from '../editor/CodeEditor.vue'
import { STATUS_LABEL, argsOf, codeHeight, firstLine, formatDuration, formatValue, outputOf, resolveStatus, resultOutput, toolNameOf } from './toolPart'
const PREFIX_META = [
    { prefix: 'browser_', label: '浏览器', icon: Globe },
    { prefix: 'word_', label: 'Word', icon: FileText },
    { prefix: 'computer_', label: '电脑', icon: Monitor },
    { prefix: 'ppt_', label: 'PPT', icon: Presentation },
    { prefix: 'excel_', label: 'Excel', icon: Sheet },
    { prefix: 'mobile_', label: '移动端', icon: Smartphone },
]
const TOOL_LABELS = {
    browser_open: '打开页面',
    browser_navigate: '导航',
    browser_navigate_back: '后退',
    browser_tabs: '标签页',
    browser_snapshot: '页面快照',
    browser_read_text: '读取文本',
    browser_take_screenshot: '截图',
    browser_click: '点击',
    browser_hover: '悬停',
    browser_type: '输入',
    browser_select_option: '选择选项',
    browser_press_key: '按键',
    browser_get_attribute: '读取属性',
    browser_scroll: '滚动',
    browser_wait_for: '等待条件',
    browser_waiting_for_user_interaction: '等待用户操作',
    browser_evaluate: '执行脚本',
    browser_handle_dialog: '处理弹窗',
    browser_console_messages: '控制台日志',
    browser_network_requests: '网络请求',
    browser_state: '浏览器状态',
    word_document: '文档操作',
    word_read: '读取内容',
    word_write: '写入内容',
    word_table: '表格操作',
    word_format: '格式设置',
    word_structure: '文档结构',
    word_find: '查找替换',
    word_edit: '编辑内容',
    word_media: '媒体对象',
    word_variable: '变量替换',
    word_batch: '批量操作',
    word_where_am_i: '当前位置',
    ppt_presentation: '演示文稿',
    ppt_read: '读取内容',
    ppt_slide: '幻灯片',
    ppt_write: '写入内容',
    ppt_shape: '形状操作',
    ppt_table: '表格操作',
    ppt_format: '格式设置',
    ppt_chart: '图表操作',
    ppt_media: '媒体对象',
    ppt_where_am_i: '当前位置',
    excel_workbook: '工作簿',
    excel_read: '读取内容',
    excel_write: '写入内容',
    excel_sheet: '工作表',
    excel_format: '格式设置',
    excel_formula: '公式操作',
    excel_table: '表格操作',
    excel_chart: '图表操作',
    excel_sort_filter: '排序筛选',
    excel_where_am_i: '当前位置',
    mobile_list_available_devices: '列出设备',
    mobile_list_apps: '列出应用',
    mobile_get_foreground_app: '前台应用',
    mobile_launch_app: '启动应用',
    mobile_terminate_app: '停止应用',
    mobile_install_app: '安装应用',
    mobile_uninstall_app: '卸载应用',
    mobile_get_screen_size: '屏幕尺寸',
    mobile_click_on_screen_at_coordinates: '点击屏幕',
    mobile_double_tap_on_screen: '双击屏幕',
    mobile_long_press_on_screen_at_coordinates: '长按屏幕',
    mobile_list_elements_on_screen: '元素树',
    mobile_press_button: '发送按键',
    mobile_open_url: '打开网址',
    mobile_swipe_on_screen: '滑动屏幕',
    mobile_type_keys: '输入文本',
    mobile_save_screenshot: '保存截图',
    mobile_take_screenshot: '截图',
    mobile_set_orientation: '设置方向',
    mobile_set_location: '设置位置',
    mobile_clipboard: '剪贴板',
    mobile_get_device_logs: '设备日志',
    mobile_get_orientation: '读取方向',
    mobile_start_screen_recording: '开始录屏',
    mobile_stop_screen_recording: '停止录屏',
    mobile_list_crashes: '崩溃列表',
    mobile_get_crash: '崩溃详情',
    mobile_batch_commands: '批量命令',
    computer_click: '点击',
    computer_double_click: '双击',
    computer_right_click: '右键点击',
    computer_drag: '拖动',
    computer_type_text: '输入文本',
    computer_press_key: '按键',
    computer_hotkey: '组合键',
    computer_scroll: '滚动',
    computer_set_value: '设置值',
    computer_move_cursor: '移动光标',
    computer_list_apps: '列出应用',
    computer_list_windows: '列出窗口',
    computer_launch_app: '启动应用',
    computer_kill_app: '结束应用',
    computer_bring_to_front: '窗口置前',
    computer_set_window_frame: '窗口尺寸位置',
    computer_invoke_menu: '调用菜单',
    computer_get_window_state: '窗口状态',
    computer_get_desktop_state: '桌面状态',
    computer_debug_window_info: '窗口调试信息',
    computer_get_accessibility_tree: '无障碍树',
    computer_get_screen_size: '屏幕尺寸',
    computer_get_cursor_position: '光标位置',
}
const props = defineProps({ part: { type: Object, required: true } })
const open = ref(false)
const toggle = () => { open.value = !open.value }
const status = computed(() => resolveStatus(props.part))
const statusText = computed(() => STATUS_LABEL[status.value] || '')
const durationText = computed(() => formatDuration(props.part.durationMs))
const rawName = computed(() => toolNameOf(props.part))
const prefixMeta = computed(() => PREFIX_META.find((item) => rawName.value.startsWith(item.prefix)) || null)
const iconOf = computed(() => (prefixMeta.value ? prefixMeta.value.icon : Wrench))
const actionLabel = computed(() => {
    const name = rawName.value
    if (!name) return ''
    if (Object.prototype.hasOwnProperty.call(TOOL_LABELS, name)) return TOOL_LABELS[name]
    return name.replace(/^(browser|word|computer|ppt|excel|mobile)_/, '')
})
const title = computed(() => {
    const prefix = prefixMeta.value ? prefixMeta.value.label : ''
    return [prefix, actionLabel.value].filter(Boolean).join(' · ') || rawName.value
})
const argsText = computed(() => formatValue(argsOf(props.part)))
const outputText = computed(() => resultOutput(outputOf(props.part)))
const resultLang = computed(() => {
    const raw = outputText.value.trim()
    if (!raw || !/^[{\[]/.test(raw)) return 'plaintext'
    try {
        JSON.parse(raw)
        return 'json'
    } catch {
        return 'plaintext'
    }
})
const summary = computed(() => {
    if (status.value === 'running') return ''
    if (status.value === 'failed') return firstLine(outputText.value) || '调用失败'
    return firstLine(outputText.value) || firstLine(argsText.value)
})
</script>

<style scoped>
.tool-card { width: 100%; max-width: 100%; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; font-size: 12px; background: var(--card); }
.tool-head { display: flex; align-items: center; gap: 6px; padding: 6px 10px; background: var(--muted); cursor: pointer; user-select: none; }
.tool-head:focus-visible { outline: 1px solid var(--primary); outline-offset: -1px; }
.tool-icon { flex: none; color: var(--muted-foreground); }
.tool-title { flex: none; max-width: 46%; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--foreground); font-weight: 600; }
.tool-status { display: inline-flex; align-items: center; gap: 3px; flex: none; color: var(--muted-foreground); }
.tool-status.running { color: var(--primary); }
.tool-status.success { color: #22c55e; }
.tool-status.failed { color: var(--destructive); }
.tool-status.pending { color: #f59e0b; }
.tool-summary { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--muted-foreground); }
.tool-duration { flex: none; color: var(--muted-foreground); font-size: 11px; font-family: Consolas, Menlo, 'Courier New', monospace; }
.arrow { flex: none; color: var(--muted-foreground); transition: transform 0.15s ease; }
.arrow.open { transform: rotate(90deg); }
.tool-body { padding: 8px 10px 10px; border-top: 1px solid var(--border); display: flex; flex-direction: column; gap: 8px; }
.tool-section { display: flex; flex-direction: column; gap: 4px; }
.tool-label { color: var(--muted-foreground); font-weight: 600; }
.tool-label.bad { color: var(--destructive); }
</style>
