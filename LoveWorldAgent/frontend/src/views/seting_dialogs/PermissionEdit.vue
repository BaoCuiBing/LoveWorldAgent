<template>
    <div class="permission-edit">
        <header class="perm-head">
            <div class="head-title">
                <span class="perm-head-title">权限</span>
                <Tooltip>
                    <TooltipTrigger as-child>
                        <span class="title-help" aria-label="权限说明"><Info :size="14" /></span>
                    </TooltipTrigger>
                    <TooltipContent>控制智能体执行终端命令、浏览器操作与 MCP 工具时的审批方式</TooltipContent>
                </Tooltip>
            </div>
        </header>
        <div class="perm-section">
            <div class="perm-title">常规任务</div>
            <RadioGroup :model-value="form.mode" class="panel-card mode-list" @update:model-value="setMode">
                <template v-for="option in modeOptions" :key="option.value">
                    <div class="mode-row" :class="{ on: form.mode === option.value }" @click="setMode(option.value)">
                        <RadioGroupItem :value="option.value" class="mode-radio" />
                        <component :is="option.icon" class="mode-icon" :class="{ warn: option.warn }" :size="18" />
                        <div class="mode-main">
                            <div class="mode-name">{{ option.label }}</div>
                            <div class="mode-desc">{{ option.desc }}</div>
                        </div>
                    </div>
                    <div v-if="option.value === 'auto' && form.mode === 'auto'" class="perm-collapse">
                        <div class="group-hint">蓝=自动放行，灰=需你批准；PowerShell 仅列高风险命令（未列默认放行，输出重定向仍需批准）；MCP 服务按整项控制，其余内置工具组关闭即拒绝执行。</div>
                        <Collapsible v-for="group in groups" :key="group.key" :open="opened.includes(group.key)" @update:open="(value) => setGroupOpen(group.key, value)">
                            <CollapsibleTrigger as-child>
                                <div class="group-head">
                                    <span class="group-name">{{ group.label }}</span>
                                    <span class="group-count">({{ enabledCount(form.groups[group.key]) }}/{{ group.commands.length }})</span>
                                    <ChevronRight class="group-arrow" :class="{ open: opened.includes(group.key) }" :size="12" />
                                    <span class="group-switch" @click.stop>
                                        <Switch :model-value="form.groups[group.key].enabled" @update:model-value="(value) => onGroupChange(group.key, value)" />
                                    </span>
                                </div>
                            </CollapsibleTrigger>
                            <CollapsibleContent>
                                <div class="group-tags">
                                    <button v-for="command in group.commands" :key="command" type="button" class="cmd-tag" :class="{ on: isCommandAllowed(form.groups[group.key], command) }" :aria-pressed="isCommandAllowed(form.groups[group.key], command)" :aria-label="`${command}，${isCommandAllowed(form.groups[group.key], command) ? '已放行' : '需审批'}`" @click="onCommandToggle(group.key, command)">{{ command }}</button>
                                </div>
                            </CollapsibleContent>
                        </Collapsible>
                    </div>
                </template>
            </RadioGroup>
        </div>
        <div v-if="form.custom.length" class="perm-section">
            <div class="perm-title">自定义白名单</div>
            <div class="panel-card custom-card">
                <div class="custom-desc">命令审批时点「永久允许」会追加到此处；命中的命令在手动审批下也不再询问。</div>
                <div class="custom-tags">
                    <Tooltip v-for="command in form.custom" :key="command">
                        <TooltipTrigger as-child>
                            <span class="custom-tag">
                                <span class="custom-tag-text">{{ command }}</span>
                                <button type="button" class="custom-tag-close" aria-label="移除" @click.stop="removeCustom(command)"><X :size="10" /></button>
                            </span>
                        </TooltipTrigger>
                        <TooltipContent>{{ command }}</TooltipContent>
                    </Tooltip>
                </div>
            </div>
        </div>
    </div>
</template>

<script setup>
import { onMounted, reactive, ref, watch } from 'vue'
import { ChevronRight, CircleCheck, Info, TriangleAlert, X } from 'lucide-vue-next'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '../../components/shadcn_ui/ui/collapsible'
import { RadioGroup, RadioGroupItem } from '../../components/shadcn_ui/ui/radio-group'
import { Switch } from '../../components/shadcn_ui/ui/switch'
import { Tooltip, TooltipContent, TooltipTrigger } from '../../components/shadcn_ui/ui/tooltip'
import { toast } from '../../components/shadcn_ui/lib/useToast'
import { getPermission, savePermission } from '../../api/request_data'
const props = defineProps({ intentMode: { type: String, default: '' } })
const emit = defineEmits(['consumed', 'saved'])
const MODES = ['manual', 'auto', 'full']
const modeOptions = [
    { value: 'manual', label: '手动审批', desc: '在隔离环境中运行。高风险操作会被阻止，重要操作需要你的批准。', icon: CircleCheck },
    { value: 'auto', label: '自动审批', desc: '在隔离环境中运行。高风险操作会由 AI 自动批准。', icon: TriangleAlert },
    { value: 'full', label: '完全访问', desc: '直接在你的电脑上运行，不使用隔离环境。它可以读写任何文件、跳过安全检查，并且不再请求批准。', icon: TriangleAlert, warn: true },
]
const groups = ref([])  // 8 组命令定义来自后端 GET /api/permission 的 groups_meta（唯一事实源）
const form = reactive({ mode: 'auto', groups: {}, custom: [] })
const opened = ref([])
const loaded = ref(false)
let pendingMode = ''
const groupCommands = (group) => Object.keys((group && group.commands) || {})  // 权限页的派生逻辑：组开关与逐条命令开关的联动（纯函数）
const enabledCount = (group) => groupCommands(group).filter((command) => group.commands[command]).length
const isCommandAllowed = (group, command) => Boolean(group && group.enabled && group.commands && group.commands[command])
const setGroupOpen = (key, value) => {
    opened.value = value ? [...opened.value, key] : opened.value.filter((item) => item !== key)
}
/** 整组开关：开/关同时批量写入该组所有命令 */
const toggleGroup = (group, enabled) => ({ enabled, commands: Object.fromEntries(groupCommands(group).map((command) => [command, enabled])) })
/** 单条开关：组开着就只翻转该条；组关着则自动开组且只放行被点的这一条 */
const toggleCommand = (group, command) => {
    const commands = { ...((group && group.commands) || {}) }
    if (group && group.enabled) {
        commands[command] = !commands[command]
        return { enabled: true, commands }
    }
    return { enabled: true, commands: Object.fromEntries(Object.keys(commands).map((item) => [item, item === command])) }
}
const normalizeGroups = (raw, defs) => Object.fromEntries(defs.map((group) => {
    const source = raw ? raw[group.key] : null
    if (typeof source === 'boolean') return [group.key, { enabled: source, commands: Object.fromEntries(group.commands.map((command) => [command, source])) }]
    const commands = (source && source.commands) || {}
    return [group.key, { enabled: Boolean(source && source.enabled), commands: Object.fromEntries(group.commands.map((command) => [command, Boolean(commands[command])])) }]
}))
const applyData = (data) => {
    if (Array.isArray(data.groups_meta)) groups.value = data.groups_meta.map((group) => ({ key: String(group.key), label: String(group.label), commands: Array.isArray(group.commands) ? group.commands : [] }))
    if (MODES.includes(data.mode)) form.mode = data.mode
    form.groups = normalizeGroups(data.groups, groups.value)
    form.custom = Array.isArray(data.custom) ? [...data.custom] : []
}
const persist = async () => {
    const snapshot = JSON.stringify({ mode: form.mode, groups: form.groups, custom: form.custom })
    try {
        await savePermission({ mode: form.mode, groups: JSON.parse(snapshot).groups, custom: [...form.custom] })
        emit('saved')  // 通知外层刷新审批 chip 等共用同一数据源的展示
    } catch (err) {
        const previous = JSON.parse(snapshot)
        form.mode = previous.mode
        form.groups = previous.groups
        form.custom = previous.custom
        toast.error(`保存权限失败：${err.message}`)
    }
}
const onGroupChange = (key, enabled) => {
    form.groups[key] = toggleGroup(form.groups[key], enabled)
    persist()
}
const onCommandToggle = (key, command) => {
    form.groups[key] = toggleCommand(form.groups[key], command)
    persist()
}
const removeCustom = (command) => {
    form.custom = form.custom.filter((item) => item !== command)
    persist()
}
const setMode = (value) => {
    if (!value || form.mode === value) return
    form.mode = value
    persist()
}
const applyIntent = () => {
    if (!props.intentMode) return
    emit('consumed')
    if (!loaded.value) {
        pendingMode = props.intentMode
        form.mode = props.intentMode
        return
    }
    if (form.mode === props.intentMode) return
    form.mode = props.intentMode
    persist()
}
watch(() => props.intentMode, applyIntent, { immediate: true })
onMounted(async () => {
    try {
        applyData(await getPermission())
    } catch (err) {
        toast.error(`读取权限失败：${err.message}`)
    } finally {
        loaded.value = true
        if (pendingMode) {
            if (form.mode !== pendingMode) {
                form.mode = pendingMode
                persist()
            }
            pendingMode = ''
        }
    }
})
</script>

<style scoped>
.permission-edit { display: flex; flex-direction: column; gap: 8px; min-height: 0; }
.perm-head { display: flex; align-items: center; justify-content: space-between; }
.head-title { display: flex; align-items: center; gap: 6px; }
.perm-head-title { font-size: 15px; font-weight: 600; }
.title-help { display: inline-flex; align-items: center; color: var(--muted-foreground); cursor: help; }
.perm-section { display: flex; flex-direction: column; gap: 6px; }
.perm-title { font-size: 12px; color: var(--muted-foreground); }
.mode-list { display: flex; flex-direction: column; gap: 6px; padding: 6px; }
.mode-row { display: flex; align-items: center; gap: 10px; padding: 12px; border: 1px solid var(--border); border-radius: 8px; cursor: pointer; }
.mode-row:hover { background: var(--accent); }
.mode-row.on { border-color: var(--primary); background: color-mix(in srgb, var(--primary) 12%, transparent); }
.mode-radio { flex: none; }
.mode-icon { flex: none; color: var(--foreground); }
.mode-icon.warn { color: #e6a23c; }
.mode-main { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.mode-name { font-size: 13px; font-weight: 600; }
.mode-desc { font-size: 12px; color: var(--muted-foreground); }
.perm-collapse { margin: -2px 0 4px 20px; }
.group-head { display: flex; align-items: center; height: 44px; padding: 0 12px; border-top: 1px solid var(--border); cursor: pointer; }
.group-name { font-size: 13px; font-weight: 600; }
.group-count { margin-left: 6px; font-size: 12px; color: var(--muted-foreground); }
.group-arrow { margin-left: 8px; color: var(--muted-foreground); transition: transform 0.2s ease; }
.group-arrow.open { transform: rotate(90deg); }
.group-switch { display: inline-flex; align-items: center; flex: none; margin-left: auto; }
.group-tags { display: flex; flex-wrap: wrap; gap: 6px; padding: 2px 12px 6px; }
.cmd-tag { padding: 0 7px; border: 1px solid var(--border); border-radius: 4px; background: transparent; color: var(--muted-foreground); font-size: 11px; line-height: 20px; cursor: pointer; user-select: none; transition: color 0.15s ease, border-color 0.15s ease, background 0.15s ease; }
.cmd-tag:hover { color: var(--foreground); border-color: var(--foreground); }
.cmd-tag.on { color: var(--primary); border-color: var(--primary); background: color-mix(in srgb, var(--primary) 12%, transparent); }
.cmd-tag:focus-visible { outline: 1px solid var(--primary); outline-offset: 1px; }
.group-hint { padding: 10px 12px 2px; font-size: 11px; line-height: 1.6; color: var(--muted-foreground); }
.custom-card { display: flex; flex-direction: column; gap: 8px; padding: 12px; }
.custom-desc { font-size: 12px; color: var(--muted-foreground); line-height: 1.6; }
.custom-tags { display: flex; flex-wrap: wrap; gap: 6px; }
.custom-tag { display: inline-flex; align-items: center; gap: 4px; max-width: 240px; padding: 0 6px; border: 1px solid #e6a23c; border-radius: 4px; color: #e6a23c; font-size: 11px; line-height: 20px; }
.custom-tag-text { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.custom-tag-close { display: inline-flex; align-items: center; justify-content: center; padding: 0; border: none; background: transparent; color: inherit; cursor: pointer; opacity: 0.8; }
.custom-tag-close:hover { opacity: 1; }
</style>
