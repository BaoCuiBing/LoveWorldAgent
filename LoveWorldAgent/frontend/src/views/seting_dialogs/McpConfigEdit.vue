<template>
    <div class="mcp-edit">
        <div class="group-label">已配置的 MCP Servers</div>
        <div class="panel-card mcp-card">
            <header class="mcp-head">
                <div class="head-main">
                    <div class="head-title">
                        <span class="mcp-title">MCP Servers 管理</span>
                    </div>
                    <p class="mcp-desc">管理您已添加的 MCP 服务器，可启用、配置或添加新的工具能力。</p>
                </div>
                <div class="head-actions">
                    <button class="mcp-icon-btn" type="button" aria-label="刷新并检测状态" :disabled="loading || probing" @click="onRefresh"><RefreshCw :size="16" :class="{ spin: probing }" /></button>
                    <DropdownMenu>
                        <DropdownMenuTrigger as-child>
                            <Button><Plus :size="14" /><span>添加</span><ChevronDown :size="14" /></Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                            <DropdownMenuItem @select="onHeadCommand('add')">从 JSON 配置添加</DropdownMenuItem>
                            <DropdownMenuItem @select="onHeadCommand('json')">JSON 编辑</DropdownMenuItem>
                        </DropdownMenuContent>
                    </DropdownMenu>
                </div>
            </header>
            <div v-if="loadError" class="mcp-error">{{ loadError }}</div>
            <div v-else-if="!loading && !items.length" class="empty-state">
                <Cable :size="36" />
                <div class="empty-title">暂无 MCP 服务</div>
                <div class="empty-hint">点击「添加」粘贴 JSON 配置接入第一个服务</div>
            </div>
            <div v-else class="mcp-list">
                <div v-for="item in items" :key="item.name" class="mcp-item">
                    <div class="mcp-row">
                        <button class="mcp-expand" type="button" :aria-label="expanded === item.name ? '收起详情' : '展开详情'" @click="expanded = expanded === item.name ? '' : item.name">
                            <span class="mcp-arrow" :class="{ open: expanded === item.name }"><ChevronRight :size="12" /></span>
                        </button>
                        <span class="mcp-avatar" :style="{ background: avatarColor(item.name) }">{{ (item.name || '?').charAt(0).toUpperCase() }}</span>
                        <span class="mcp-name">{{ item.name }}</span>
                        <Tooltip>
                            <TooltipTrigger as-child>
                                <span class="mcp-status" :class="statusOf(item).tone">
                                    <component :is="statusOf(item).icon" :size="13" :class="{ spin: statusOf(item).spin }" />
                                    <span>{{ statusOf(item).label }}</span>
                                </span>
                            </TooltipTrigger>
                            <TooltipContent>{{ statusTitle(item) }}</TooltipContent>
                        </Tooltip>
                        <Switch :model-value="item.enabled" @update:model-value="(value) => onToggle(item, value)" />
                    </div>
                    <div v-if="expanded === item.name" class="mcp-detail">
                        <div v-for="row in detailRows(item)" :key="row.label" class="mcp-detail-row"><span class="mcp-detail-label">{{ row.label }}</span><span class="mcp-detail-value">{{ row.value }}</span></div>
                        <div class="mcp-detail-actions">
                            <Button size="sm" variant="outline" @click="onEdit(item)">编辑</Button>
                            <Button size="sm" variant="destructive" @click="onDelete(item)">删除</Button>
                        </div>
                    </div>
                </div>
            </div>
        </div>

        <Dialog v-model:open="addOpen">
            <DialogContent class="w-[620px] max-w-[calc(100vw-32px)]">
                <DialogHeader>
                    <DialogTitle>手动配置</DialogTitle>
                    <DialogDescription class="sr-only">粘贴 MCP Server 的 JSON 配置</DialogDescription>
                </DialogHeader>
                <div class="dialog-head">
                    <span class="dialog-hint">请从 MCP Servers 的介绍页面复制配置 JSON（优先使用 NPX 或 UVX 配置），并粘贴到输入框中。</span>
                </div>
                <CodeEditor v-model="addText" language="json" height="320px" />
                <div class="dialog-foot">
                    <span class="dialog-warn"><TriangleAlert :size="14" />配置前请确认来源，甄别风险</span>
                    <div class="dialog-buttons"><Button size="sm" variant="outline" @click="addOpen = false">取消</Button><Button size="sm" :disabled="saving" @click="onSaveAdd">确认</Button></div>
                </div>
            </DialogContent>
        </Dialog>

        <Dialog v-model:open="jsonOpen">
            <DialogContent class="w-[680px] max-w-[calc(100vw-32px)]">
                <DialogHeader>
                    <DialogTitle>JSON编辑</DialogTitle>
                    <DialogDescription class="sr-only">直接编辑 agent_mcp.json</DialogDescription>
                </DialogHeader>
                <div class="dialog-head"><span class="dialog-hint">直接编辑 agent_mcp.json，保存时逐项校验；env/headers 按真实值显示并原样保存。</span></div>
                <CodeEditor v-model="jsonText" language="json" height="360px" />
                <div class="dialog-foot">
                    <span class="dialog-warn"><TriangleAlert :size="14" />错误的配置会导致服务无法连接</span>
                    <div class="dialog-buttons"><Button size="sm" variant="outline" @click="jsonOpen = false">取消</Button><Button size="sm" :disabled="saving" @click="onSaveJson">保存</Button></div>
                </div>
            </DialogContent>
        </Dialog>

        <Dialog v-model:open="editorOpen">
            <DialogContent class="w-[640px] max-w-[calc(100vw-32px)]">
                <DialogHeader>
                    <DialogTitle>编辑 MCP 服务（JSON）</DialogTitle>
                    <DialogDescription class="sr-only">按 mcpServers 结构编辑该服务</DialogDescription>
                </DialogHeader>
                <div class="dialog-head"><span class="dialog-hint">按 agent_mcp.json 的 mcpServers 结构编辑该服务；对象键即服务名，改名会同步重命名。</span></div>
                <CodeEditor v-model="editText" language="json" height="360px" />
                <div class="dialog-foot">
                    <span class="dialog-warn"><TriangleAlert :size="14" />错误的配置会导致服务无法连接</span>
                    <div class="dialog-buttons"><Button size="sm" variant="outline" @click="editorOpen = false">取消</Button><Button size="sm" :disabled="saving" @click="onSave">保存</Button></div>
                </div>
            </DialogContent>
        </Dialog>
    </div>
</template>

<script setup>
import { onActivated, onDeactivated, onMounted, onUnmounted, ref } from 'vue'
import { Cable, ChevronDown, ChevronRight, CircleCheckBig, CircleX, Info, LoaderCircle, Plus, RefreshCw, TriangleAlert } from 'lucide-vue-next'
import { Button } from '../../components/shadcn_ui/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '../../components/shadcn_ui/ui/dialog'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '../../components/shadcn_ui/ui/dropdown-menu'
import { Switch } from '../../components/shadcn_ui/ui/switch'
import { Tooltip, TooltipContent, TooltipTrigger } from '../../components/shadcn_ui/ui/tooltip'
import { toast } from '../../components/shadcn_ui/lib/useToast'
import { confirm } from '../../components/shadcn_ui/lib/useConfirm'
import CodeEditor from '../../components/editor/CodeEditor.vue'
import { deleteMcp, getMcpJson, getMcpStatus, listMcp, saveMcpJson, setMcpEnabled, updateMcp } from '../../api/request_data'
const ADD_TEMPLATE = `// 示例：
// {
//   "mcpServers": {
//     "example-server": {
//       "command": "npx",
//       "args": [
//         "-y",
//         "mcp-server-example"
//       ]
//     }
//   }
// }

`
const AVATAR_COLORS = ['#1f9be0', '#20c4c4', '#f2787c', '#b06ae0', '#4d8ce8', '#e0964a', '#59b86b', '#d067b0']
const NAME_RE = /^[\w\u4e00-\u9fa5][\w\u4e00-\u9fa5 .-]{0,49}$/
const items = ref([])
const loading = ref(false)
const loadError = ref('')
const saving = ref(false)
const expanded = ref('')
const addOpen = ref(false)
const jsonOpen = ref(false)
const addText = ref(ADD_TEMPLATE)
const jsonText = ref('')
const editorOpen = ref(false)
const editId = ref('')
const editText = ref('')
const avatarColor = (name) => {
    let hash = 0
    for (const ch of String(name || '')) hash = (hash * 31 + ch.charCodeAt(0)) >>> 0
    return AVATAR_COLORS[hash % AVATAR_COLORS.length]
}
const typeLabel = (type) => ({ stdio: '本地进程', sse: 'SSE', http: 'HTTP' }[type] || type)
const pairsText = (value) => Object.entries(value && typeof value === 'object' && !Array.isArray(value) ? value : {}).map(([key, val]) => `${key}=${val}`).join(', ') || '—'
const detailRows = (item) => {
    const rows = [{ label: '连接类型', value: typeLabel(item.type) }, { label: '状态', value: item.enabled ? '已启用' : '已停用' }]
    if (item.type === 'stdio') {
        rows.push({ label: '启动命令', value: [item.command, ...(item.args || [])].filter(Boolean).join(' ') || '—' })
        rows.push({ label: '环境变量', value: pairsText(item.env) })
    } else {
        rows.push({ label: '服务地址', value: item.url || '—' })
        rows.push({ label: '请求头', value: pairsText(item.headers) })
    }
    return rows
}
const loadList = async () => {
    loading.value = true
    try {
        const data = await listMcp()
        items.value = data.list || []
        loadError.value = ''
    } catch (err) {
        items.value = []
        loadError.value = `读取 MCP 配置失败：${err.message}`
    } finally {
        loading.value = false
    }
}
const statuses = ref({})
const probing = ref(false)
const POLL_INTERVAL = 5000
const STATUS_META = {
    connected: { label: '已连接', tone: 'ok', icon: CircleCheckBig },
    connecting: { label: '连接中', tone: 'busy', icon: LoaderCircle, spin: true },
    error: { label: '连接失败', tone: 'error', icon: TriangleAlert },
    disabled: { label: '未启用', tone: 'off', icon: CircleX },
    unknown: { label: '未检测', tone: 'off', icon: Info },
}
const statusOf = (item) => {
    const hit = statuses.value[item.name]
    if (hit) {
        const meta = STATUS_META[hit.status] || STATUS_META.unknown
        return { ...meta, label: hit.status === 'connected' ? `${meta.label} ${hit.tools || 0}` : meta.label, kind: hit.status }
    }
    if (!item.enabled) return { ...STATUS_META.disabled, kind: 'disabled' }
    if (probing.value) return { ...STATUS_META.connecting, label: '检测中', kind: 'connecting' }
    return { ...STATUS_META.unknown, kind: 'unknown' }
}
const statusTitle = (item) => {
    const hit = statuses.value[item.name]
    if (!hit) return item.enabled ? '尚未连接，等待自动刷新或点击右上角刷新' : '已停用，不会被连接'
    if (hit.status === 'connected') return `已连接，返回 ${hit.tools || 0} 个工具`
    if (hit.status === 'connecting') return '正在连接并读取工具列表…'
    if (hit.status === 'error') return hit.error || '连接失败'
    return '未启用'
}
const loadStatus = async ({ quiet = false, refresh = false } = {}) => {
    if (!quiet) probing.value = true
    try {
        const data = await getMcpStatus('', refresh)
        statuses.value = data.list || {}
    } catch (err) {
        if (!quiet) toast.error(`刷新 MCP 状态失败：${err.message}`)
    } finally {
        if (!quiet) probing.value = false
    }
}
const onRefresh = async () => {
    await loadList()
    await loadStatus({ refresh: true })
}
let pollTimer = null
const stopPolling = () => {
    if (pollTimer) { clearInterval(pollTimer); pollTimer = null }
}
const startPolling = () => {
    stopPolling()
    pollTimer = setInterval(() => loadStatus({ quiet: true }), POLL_INTERVAL)
}
let booted = false
const openAdd = () => { addText.value = ADD_TEMPLATE; addOpen.value = true }
const openJson = async () => {
    try {
        jsonText.value = (await getMcpJson()).text || '{ "servers": [] }\n'
        jsonOpen.value = true
    } catch (err) {
        toast.error(`读取配置失败：${err.message}`)
    }
}
const onHeadCommand = (command) => (command === 'json' ? openJson() : openAdd())
const stripComments = (text) => String(text || '').split(/\r?\n/).map((line) => (/^\s*(\/\/|#)/.test(line) ? '' : line)).join('\n')
const pairsOf = (value) => Object.fromEntries(Object.entries(value && typeof value === 'object' && !Array.isArray(value) ? value : {}).map(([key, val]) => [key, String(val)]))
const parseConfig = (text) => {
    let parsed
    try { parsed = JSON.parse(stripComments(text)) } catch (err) { throw new Error(`JSON 格式错误：${err.message}`) }
    const wrap = (name, value) => {
        const item = value && typeof value === 'object' && !Array.isArray(value) ? value : {}
        const declared = String(item.type || '').toLowerCase()
        const kind = ['stdio', 'sse', 'http'].includes(declared) ? declared : (item.url ? 'http' : 'stdio')
        return {
            name: String(name || item.name || '').trim(), type: kind, command: String(item.command || ''),
            args: Array.isArray(item.args) ? item.args.map(String) : [], env: pairsOf(item.env),
            url: String(item.url || ''), headers: pairsOf(item.headers),
            enabled: item.enabled !== false && item.disabled !== true,
        }
    }
    if (parsed && typeof parsed.mcpServers === 'object' && !Array.isArray(parsed.mcpServers)) {
        return Object.entries(parsed.mcpServers).map(([name, value]) => wrap(name, value))
    }
    if (Array.isArray(parsed && parsed.servers)) return parsed.servers.map((item) => wrap(item && item.name, item))
    if (parsed && typeof parsed === 'object' && (parsed.command || parsed.url)) return [wrap(parsed.name, parsed)]
    throw new Error('请使用 { "mcpServers": { "服务名": { ... } } } 格式')
}
const onSaveAdd = async () => {
    let entries
    try { entries = parseConfig(addText.value) } catch (err) { toast.warning(err.message); return }
    if (!entries.length) { toast.warning('请填写至少一个 MCP 服务'); return }
    for (const item of entries) {
        if (!NAME_RE.test(item.name)) { toast.warning(`名称「${item.name}」需以字母开头，仅含字母、数字、下划线与横线`); return }
        if (item.type === 'stdio' && !item.command) { toast.warning(`「${item.name}」缺少启动命令 command`); return }
        if (item.type !== 'stdio' && !/^https?:\/\//i.test(item.url)) { toast.warning(`「${item.name}」缺少 http(s) 开头的 url`); return }
    }
    saving.value = true
    try {
        const current = JSON.parse((await getMcpJson()).text || '{ "servers": [] }')
        const map = new Map((Array.isArray(current.servers) ? current.servers : []).filter((item) => item && item.name).map((item) => [item.name, item]))
        entries.forEach((item) => {
            const old = map.get(item.name) || {}
            const merged = { ...old, ...item }
            if (!Object.keys(item.env).length && old.env) merged.env = old.env
            if (!Object.keys(item.headers).length && old.headers) merged.headers = old.headers
            map.set(item.name, merged)
        })
        await saveMcpJson(JSON.stringify({ servers: [...map.values()] }, null, 2) + '\n')
        addOpen.value = false
        toast.success(`已添加 ${entries.length} 个 MCP 服务`)
        statuses.value = {}
        await loadList()
        loadStatus({ quiet: true })
    } catch (err) {
        toast.error(`保存失败：${err.message}`)
    } finally { saving.value = false }
}
const onSaveJson = async () => {
    saving.value = true
    try {
        await saveMcpJson(jsonText.value)
        jsonOpen.value = false
        toast.success('MCP 配置已保存')
        statuses.value = {}
        await loadList()
        loadStatus({ quiet: true })
    } catch (err) {
        toast.error(`保存失败：${err.message}`)
    } finally { saving.value = false }
}
const configOf = (item) => {
    const config = { type: item.type }
    if (item.type === 'stdio') {
        config.command = item.command || ''
        config.args = item.args || []
    } else {
        config.url = item.url || ''
    }
    if (Object.keys(item.env || {}).length) config.env = item.env
    if (Object.keys(item.headers || {}).length) config.headers = item.headers
    config.enabled = item.enabled !== false
    Object.keys(item).forEach((key) => { if (!['name', 'type', 'command', 'args', 'env', 'url', 'headers', 'enabled', 'disabled'].includes(key)) config[key] = item[key] })
    return config
}
const onEdit = (item) => {
    editId.value = item.name
    editText.value = JSON.stringify({ [item.name]: configOf(item) }, null, 2) + '\n'
    editorOpen.value = true
}
const onDelete = async (item) => {
    const ok = await confirm({ title: '删除 MCP 服务', description: `删除 MCP 服务「${item.name}」？该操作不可撤销。`, confirmText: '删除', destructive: true })
    if (!ok) return
    try {
        await deleteMcp(item.name)
        delete statuses.value[item.name]
        toast.success('已删除 MCP 服务')
        await loadList()
        loadStatus({ quiet: true })
    } catch (err) {
        toast.error(`删除失败：${err.message}`)
    }
}
const onToggle = async (item, value) => {
    const previous = item.enabled
    item.enabled = value
    try {
        await setMcpEnabled(item.name, value)
        statuses.value = { ...statuses.value, [item.name]: { status: value ? 'connecting' : 'disabled', tools: 0, error: value ? '' : '未启用', updated_at: 0 } }
        loadStatus({ quiet: true })
    } catch (err) {
        item.enabled = previous
        toast.error(`切换失败：${err.message}`)
    }
}
const onSave = async () => {
    let parsed
    try { parsed = JSON.parse(stripComments(editText.value)) } catch (err) { toast.warning(`JSON 格式错误：${err.message}`); return }
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) { toast.warning('请使用 { "服务名": { ... } } 结构'); return }
    const names = Object.keys(parsed)
    if (names.length !== 1) { toast.warning('请只保留一个 MCP 服务'); return }
    const name = names[0].trim()
    if (!NAME_RE.test(name)) { toast.warning(`名称「${name}」需以字母、数字或中文开头，仅含中英文、数字、空格与 ._-`); return }
    const raw = parsed[names[0]]
    if (!raw || typeof raw !== 'object' || Array.isArray(raw)) { toast.warning('服务配置必须是对象'); return }
    const payload = { ...raw, name }
    const type = String(payload.type || (payload.url ? 'http' : 'stdio')).toLowerCase()
    if (type === 'stdio' && !String(payload.command || '').trim()) { toast.warning('stdio 类型需要 command'); return }
    if (type !== 'stdio' && !/^https?:\/\//i.test(String(payload.url || ''))) { toast.warning('http/sse 类型需要 http(s) 开头的 url'); return }
    saving.value = true
    try {
        await updateMcp(editId.value, payload)
        editorOpen.value = false
        toast.success('已保存 MCP 服务')
        statuses.value = {}
        await loadList()
        loadStatus({ quiet: true })
    } catch (err) { toast.error(`保存失败：${err.message}`) } finally { saving.value = false }
}
onMounted(async () => {
    await loadList()
    await loadStatus()
    booted = true
    startPolling()
})
onActivated(() => {
    if (booted) loadStatus({ quiet: true })
    startPolling()
})
onDeactivated(stopPolling)
onUnmounted(stopPolling)
</script>

<style scoped>
.mcp-edit { display: flex; flex-direction: column; gap: 8px; min-height: 0; }
.group-label { font-size: 15px; font-weight: 600; color: var(--foreground); }
.mcp-card { padding: 12px; }
.mcp-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; }
.head-main { min-width: 0; }
.head-title { display: flex; align-items: center; gap: 6px; }
.mcp-title { font-size: 15px; font-weight: 600; }
.mcp-desc { margin: 4px 0 0; font-size: 12px; color: var(--muted-foreground); }
.head-actions { display: flex; flex: none; align-items: center; gap: 8px; }
.mcp-icon-btn { display: inline-flex; align-items: center; justify-content: center; width: 30px; height: 30px; border: 1px solid var(--border); border-radius: 6px; background: var(--accent); color: var(--foreground); cursor: pointer; }
.mcp-icon-btn:hover { color: var(--primary); border-color: var(--primary); }
.mcp-icon-btn:disabled { opacity: .5; cursor: not-allowed; }
.mcp-list { display: flex; flex-direction: column; gap: 2px; margin-top: 12px; max-height: 320px; overflow-y: auto; }
.mcp-item { border-top: 1px solid var(--border); }
.mcp-row { display: flex; align-items: center; gap: 12px; height: 48px; padding: 0 6px; border-radius: 6px; }
.mcp-row:hover { background: var(--accent); }
.mcp-expand { display: inline-flex; align-items: center; justify-content: center; width: 18px; height: 18px; border: none; background: transparent; color: var(--muted-foreground); cursor: pointer; }
.mcp-arrow { display: inline-flex; transition: transform .15s ease; }
.mcp-arrow.open { transform: rotate(90deg); }
.mcp-avatar { display: inline-flex; flex: none; align-items: center; justify-content: center; width: 30px; height: 30px; border-radius: 8px; color: #fff; font-size: 15px; font-weight: 700; }
.mcp-name { flex: 1; min-width: 0; overflow: hidden; font-size: 13px; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
.mcp-status { display: inline-flex; flex: none; align-items: center; gap: 4px; font-size: 11px; line-height: 1; white-space: nowrap; cursor: default; }
.mcp-status.ok { color: #67c23a; }
.mcp-status.warn { color: #e6a23c; }
.mcp-status.error { color: var(--destructive); }
.mcp-status.busy { color: var(--primary); }
.mcp-status.off { color: var(--muted-foreground); }
.spin { animation: mcp-spin 1s linear infinite; }
@keyframes mcp-spin { to { transform: rotate(360deg); } }
.mcp-detail { display: flex; flex-direction: column; gap: 6px; padding: 4px 12px 12px 36px; }
.mcp-detail-row { display: flex; gap: 12px; font-size: 12px; }
.mcp-detail-label { flex: none; width: 72px; color: var(--muted-foreground); }
.mcp-detail-value { min-width: 0; overflow-wrap: anywhere; color: var(--foreground); }
.mcp-detail-actions { display: flex; gap: 8px; margin-top: 4px; }
.mcp-error { padding: 20px 12px; font-size: 12px; color: var(--destructive); }
.empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; padding: 40px 16px; color: var(--muted-foreground); }
.empty-title { font-size: 14px; font-weight: 600; color: var(--foreground); }
.empty-hint { font-size: 12px; }
.dialog-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; margin-bottom: 10px; }
.dialog-hint { font-size: 12px; line-height: 1.7; color: var(--muted-foreground); }
.dialog-foot { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-top: 12px; }
.dialog-warn { display: inline-flex; align-items: center; gap: 4px; font-size: 11px; color: #e6a23c; }
.dialog-buttons { display: flex; gap: 8px; }
</style>
