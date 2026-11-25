<template>
    <div class="terminal-panel">
        <div class="term-tabs">
            <div ref="tabsScrollRef" class="term-tabs-scroll">
                <div v-for="tab in tabs" :key="tab.id" class="term-tab" :class="{ active: tab.id === activeId }" :data-tab-id="tab.id" @click="selectTab(tab.id)">
                    <span class="term-tab-title">{{ tab.title }}</span>
                    <span v-if="tab.cwd" class="term-tab-cwd">{{ tab.cwd }}</span>
                    <button class="term-tab-close" type="button" title="关闭" @click.stop="closeTab(tab.id)">
                        <X :size="12" />
                    </button>
                </div>
            </div>
            <button class="term-tab-add" type="button" title="新建终端" @click="addTab">+</button>
            <div class="term-tabs-actions">
                <button class="term-btn" type="button" @click="clearActive">清屏</button>
                <button class="term-btn" type="button" @click="restartActive">重启</button>
            </div>
        </div>
        <div class="term-body">
            <div v-for="tab in tabs" :key="tab.id" v-show="tab.id === activeId" class="term-host" :ref="(el) => setHost(el, tab.id)"></div>
            <div v-if="activeTab && !activeTab.connected" class="term-empty">
                <span>{{ activeTab.errorText }}</span>
                <button class="term-retry" type="button" @click="retryActive">重试</button>
            </div>
        </div>
    </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { X } from 'lucide-vue-next'
import { toast } from '../../components/shadcn_ui/lib/useToast'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { terminalWsUrl } from '../../api/request_data'
import '@xterm/xterm/css/xterm.css'
const STORE_KEY = 'lwa:ui.terminal.tabs'
const RETRY_DELAYS = [1000, 2000, 4000, 4000, 4000]  // 断线重连退避（最多 5 次，之后只能手动重试）
const INIT_COLS = 120
const INIT_ROWS = 30
const START_HINT = '正在启动终端（首次可能约 10 秒）…'
const cleanCwd = (value) => String(value || '').replace(/^\\\\\?\\/, '')  // 旧版本持久化过 verbatim 前缀 \\?\，恢复时清理
const themeOf = () => (document.documentElement.classList.contains('dark')
    ? { background: '#1e1e1e', foreground: '#d4d4d4', cursor: '#d4d4d4' }
    : { background: '#ffffff', foreground: '#1e1e1e', cursor: '#1e1e1e' })
const genId = () => 't_' + Math.random().toString(36).slice(2, 10)
const tabs = ref([])
const activeId = ref('')
const tabsScrollRef = ref(null)
const hosts = {}
const instances = {} // id -> { term, fitAddon, ws, retries, closing, pendingInput, resizeTimer, resizeObserver, themeObserver }
let saveTimer = null
const activeTab = computed(() => tabs.value.find((t) => t.id === activeId.value) || null)
const instOf = (tab) => instances[typeof tab === 'string' ? tab : (tab && tab.id)] || null
const setHost = (el, id) => {
    if (el) hosts[id] = el
    else delete hosts[id]
}
const persist = () => {
    clearTimeout(saveTimer)
    saveTimer = setTimeout(() => {
        // 只持久化用户自己创建的标签（智能体标签为临时），activeIndex 必须在这个子集里算，否则刷新后越界
        const saved = tabs.value.filter((t) => t.owned !== false)
        let idx = saved.findIndex((t) => t.id === activeId.value)
        if (idx < 0) idx = Math.max(saved.length - 1, 0)
        const data = {
            activeIndex: idx,
            tabs: saved.map((t) => ({ cwd: t.cwd })),
        }
        try { localStorage.setItem(STORE_KEY, JSON.stringify(data)) } catch { /* 隐私模式忽略 */ }
    }, 200)
}
const loadStore = () => {
    try {
        const raw = localStorage.getItem(STORE_KEY)
        if (!raw) return null
        const parsed = JSON.parse(raw)
        if (parsed && Array.isArray(parsed.tabs)) return parsed
    } catch { /* 损坏则重建 */ }
    return null
}
const sendFrame = (tab, payload) => {
    const inst = instOf(tab)
    if (!inst || !inst.ws || inst.ws.readyState !== WebSocket.OPEN) return false
    inst.ws.send(JSON.stringify(payload))
    return true
}
const flushPending = (tab) => {
    const inst = instOf(tab)
    if (!inst || !inst.pendingInput) return
    const text = inst.pendingInput
    inst.pendingInput = ''
    sendFrame(tab, { type: 'input', data: text })
}
const stopReconnect = (inst) => {
    if (inst && inst.retryTimer) { clearTimeout(inst.retryTimer); inst.retryTimer = null }
}
const scheduleReconnect = (tab) => {
    const inst = instOf(tab)
    if (!inst || inst.closing || tab.ended) return
    stopReconnect(inst)
    if (inst.retries >= RETRY_DELAYS.length) {
        tab.connected = false
        tab.errorText = '终端连接已断开，点击「重试」重新连接'
        return
    }
    const delay = RETRY_DELAYS[inst.retries]
    inst.retries += 1
    tab.connected = false
    tab.errorText = `终端连接已断开，${Math.round(delay / 1000)}s 后自动重连（第 ${inst.retries}/${RETRY_DELAYS.length} 次）`
    inst.retryTimer = setTimeout(() => openSocket(tab), delay)
}
const handleFrame = (tab, frame) => {
    const inst = instOf(tab)
    if (!inst || !frame || typeof frame !== 'object') return
    const type = String(frame.type || '')
    if (type === 'open') {
        tab.sessionId = String(frame.id || tab.sessionId || '')
        if (frame.cwd) tab.cwd = String(frame.cwd)
        tab.connected = true  // 服务端已建好会话，此后即使暂无输出也算已连接（不再无条件置真）
        tab.errorText = ''
        inst.retries = 0
        flushPending(tab)
        inst.term.focus()
        nextTick(() => { try { inst.fitAddon.fit() } catch { /* 容器不可见 */ } })
        persist()
        return
    }
    if (type === 'replay' || type === 'data') {
        if (frame.data !== undefined) inst.term.write(String(frame.data))
        tab.connected = true
        return
    }
    if (type === 'exit') {
        tab.ended = true
        tab.connected = false
        tab.errorText = '终端会话已结束'
        inst.closing = true
        stopReconnect(inst)
        if (tab.owned === false) tab.sessionId = ''  // 智能体/外部会话结束（多为后端空闲回收）：丢掉旧 id，点「重试」直接新建一条，不再先撞一次「会话不存在」
        closeSocket(tab)
        return
    }
    if (type === 'error') {
        const message = String(frame.message || '终端连接失败')
        stopReconnect(inst)
        if (/不存在/.test(message) && tab.sessionId) { // 后端已重启/会话被回收：丢掉旧 id 再开一条新会话
            closeSocket(tab)
            tab.sessionId = ''
            tab.ended = false
            inst.closing = false
            inst.retries = 0
            tab.connected = false
            tab.errorText = START_HINT
            openSocket(tab)
            return
        }
        inst.closing = true
        closeSocket(tab)
        tab.connected = false
        tab.errorText = message // 透传后端文案（如会话数上限 429）
        toast.error(message)
    }
}
const closeSocket = (tab) => {
    const inst = instOf(tab)
    if (!inst || !inst.ws) return
    const ws = inst.ws
    inst.ws = null
    ws.onopen = ws.onmessage = ws.onclose = ws.onerror = null
    try { ws.close() } catch { /* 已关闭 */ }
}
const openSocket = (tab) => {
    const inst = instOf(tab)
    if (!inst || inst.closing) return
    closeSocket(tab)
    const reuse = Boolean(tab.sessionId)
    const ws = new WebSocket(terminalWsUrl(reuse ? `id=${encodeURIComponent(tab.sessionId)}` : ''))
    inst.ws = ws
    ws.onopen = () => {
        if (reuse) return
        sendFrame(tab, { type: 'init', cols: inst.term.cols || INIT_COLS, rows: inst.term.rows || INIT_ROWS, cwd: tab.cwd || undefined })
    }
    ws.onmessage = (event) => {
        let frame = null
        try { frame = JSON.parse(String(event.data || '{}')) } catch { return /* 忽略坏帧 */ }
        handleFrame(tab, frame)
    }
    ws.onclose = () => { if (inst.ws === ws) scheduleReconnect(tab) }
    ws.onerror = () => { if (inst.ws === ws) scheduleReconnect(tab) }
}
const onResize = (tab) => {
    const inst = instOf(tab)
    if (!inst) return
    clearTimeout(inst.resizeTimer)
    inst.resizeTimer = setTimeout(() => {
        try { inst.fitAddon.fit() } catch { /* 容器不可见 */ }
        sendFrame(tab, { type: 'resize', cols: inst.term.cols, rows: inst.term.rows })
    }, 200)
}
const ensureInstance = async (tab) => {
    if (!tab) return
    if (instances[tab.id]) return
    for (let tries = 0; tries < 20 && !hosts[tab.id]; tries++) {
        await new Promise((resolve) => setTimeout(resolve, 50))
        await nextTick()
    }
    if (!hosts[tab.id]) {
        tab.connected = false
        tab.errorText = '终端容器未就绪，请重试'
        return
    }
    const term = new Terminal({ fontSize: 12, fontFamily: 'Consolas, Menlo, "Courier New", monospace', cursorBlink: true, theme: themeOf() })
    const fitAddon = new FitAddon()
    term.loadAddon(fitAddon)
    term.open(hosts[tab.id])
    try { fitAddon.fit() } catch { }
    instances[tab.id] = { term, fitAddon, ws: null, retries: 0, closing: false, pendingInput: '', resizeTimer: null, resizeObserver: null, themeObserver: null }
    const inst = instances[tab.id]
    term.onData((data) => {
        if (!sendFrame(tab, { type: 'input', data })) inst.pendingInput = ''  // 未连上时丢弃零散按键，避免重连后突然补一大段
    })
    const to = new MutationObserver(() => { term.options.theme = themeOf() })
    to.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
    inst.themeObserver = to
    const ro = new ResizeObserver(() => onResize(tab))
    ro.observe(hosts[tab.id])
    inst.resizeObserver = ro
    tab.connected = false
    tab.errorText = START_HINT
    if (tab.command) inst.pendingInput = String(tab.command) + '\r'  // 连上后的 open 帧里统一补发（runCommand 的待执行命令）
    openSocket(tab)
}
const reconnect = async (tab) => {
    await ensureInstance(tab)
    if (instOf(tab) && !tab.connected) openSocket(tab)
}
const retryActive = async () => {
    const tab = activeTab.value
    if (!tab) return
    const inst = instOf(tab)
    if (!inst) { await ensureInstance(tab); return }
    inst.closing = false
    inst.retries = 0
    tab.ended = false
    tab.errorText = START_HINT
    openSocket(tab)
}
const scrollActiveIntoView = async () => {
    await nextTick()
    const box = tabsScrollRef.value
    if (!box) return
    const target = box.querySelector(`[data-tab-id="${activeId.value}"]`)
    if (!target) return
    target.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'smooth' })  // 只让标签区横向滚动，尽量不影响页面纵向位置
}
const onTabsWheel = (event) => {
    const box = tabsScrollRef.value
    if (!box || box.scrollWidth <= box.clientWidth) return  // 无溢出时不拦截，避免劫持页面滚动
    const delta = event.shiftKey ? event.deltaY : (event.deltaY || event.deltaX)
    if (!delta) return
    event.preventDefault()
    box.scrollLeft += delta
}
const selectTab = async (id) => {
    activeId.value = id
    persist()
    await scrollActiveIntoView()
    const tab = tabs.value.find((t) => t.id === id)
    if (!tab) return
    if (instances[tab.id] && !tab.connected) { await reconnect(tab); return } // 切回未连接的标签时重连（恢复的旧标签首次点击）
    await ensureInstance(tab)
}
const addTab = async (init) => {
    const base = init || {}
    // 必须是 reactive 对象：push 进数组后局部变量仍是原始对象，直接改 connected/errorText 不会触发重渲染（“正在…”遮罩不消失的根因）
    const tab = reactive({
        id: genId(),
        title: '终端',
        cwd: cleanCwd(base.cwd),
        sessionId: base.sessionId || '',
        owned: base.sessionId ? false : true,
        connected: false,
        ended: false,
        errorText: '正在连接…',
        command: base.command || '',
    })
    tabs.value.push(tab)
    activeId.value = tab.id
    persist()
    await scrollActiveIntoView()
    await ensureInstance(tab)
    return tab
}
const dropTab = async (id, kill) => {
    const tab = tabs.value.find((t) => t.id === id)
    const inst = instOf(id)
    if (inst) {
        inst.closing = true
        stopReconnect(inst)
        if (kill) sendFrame({ id }, { type: 'close' })  // 只有用户自己的标签才回收后端 PTY
        closeSocket(tab || { id })
        if (inst.resizeObserver) inst.resizeObserver.disconnect()
        if (inst.themeObserver) inst.themeObserver.disconnect()
        inst.term.dispose()
        delete instances[id]
    }
    const idx = tabs.value.findIndex((t) => t.id === id)
    if (idx >= 0) tabs.value.splice(idx, 1)
    if (activeId.value === id) {
        const next = tabs.value[Math.max(0, idx - 1)]
        activeId.value = next ? next.id : ''
    }
}
const closeTab = async (id) => {
    const tab = tabs.value.find((t) => t.id === id)
    await dropTab(id, Boolean(tab && tab.owned !== false && tab.sessionId))
    if (!tabs.value.length) { await addTab(); return }
    persist()
    if (activeId.value) { await nextTick(); await ensureInstance(tabs.value.find((t) => t.id === activeId.value)) }
}
const clearActive = () => { const inst = instOf(activeId.value); if (inst) inst.term.clear() }
const restartActive = async () => {
    const tab = activeTab.value
    const inst = instOf(tab)
    if (!tab || !inst) return
    const keepSession = tab.owned === false  // 外部/智能体会话只重挂输出流，不关闭后端 PTY
    inst.closing = true
    stopReconnect(inst)
    closeSocket(tab)
    inst.term.reset()
    if (!keepSession && tab.sessionId) sendFrame(tab, { type: 'close' })
    if (!keepSession) tab.sessionId = ''
    inst.closing = false
    inst.retries = 0
    tab.ended = false
    tab.connected = false
    tab.errorText = START_HINT
    openSocket(tab)
}
const attachSession = async (sessionId, cwd) => {
    const sid = String(sessionId || '').trim()
    if (!sid) return null
    const existed = tabs.value.find((t) => t.sessionId === sid)
    if (existed) {
        activeId.value = existed.id
        persist()
        await nextTick()
        await ensureInstance(existed)
        if (!existed.connected) openSocket(existed)  // 已断开过就重挂一次，命令输出继续可见
        return existed
    }
    return addTab({ cwd: cwd || '', sessionId: sid })  // 标题统一「终端」，目录名只在 cwd 处展示
}
const runCommand = async (command, cwd) => {
    if (!command) return
    await addTab({ cwd: cwd || '', command: String(command) })  // 命令内容不进标题，避免标签数量与标题漂移
}
onMounted(async () => {
    if (tabsScrollRef.value) tabsScrollRef.value.addEventListener('wheel', onTabsWheel, { passive: false })  // 需 preventDefault 才能只横滚标签区
    const stored = loadStore()
    if (stored && stored.tabs.length) {
        tabs.value = stored.tabs.map((t) => ({ id: genId(), title: '终端', cwd: cleanCwd(t.cwd), sessionId: '', owned: true, connected: false, ended: false, errorText: '终端未连接', command: '' }))
        const idx = Math.min(Math.max(stored.activeIndex || 0, 0), tabs.value.length - 1)
        activeId.value = tabs.value[idx].id
        await nextTick()
        await ensureInstance(tabs.value[idx])
    } else {
        await addTab()
    }
})
onBeforeUnmount(() => {
    if (tabsScrollRef.value) tabsScrollRef.value.removeEventListener('wheel', onTabsWheel)
    clearTimeout(saveTimer)
    tabs.value.forEach((tab) => { if (tab.owned !== false && tab.sessionId) sendFrame(tab, { type: 'close' }) }) // 卸载（收起右栏）时只释放自己创建的会话，外部/智能体会话保留
    Object.values(instances).forEach((inst) => {
        inst.closing = true
        stopReconnect(inst)
        if (inst.ws) { const ws = inst.ws; inst.ws = null; ws.onopen = ws.onmessage = ws.onclose = ws.onerror = null; try { ws.close() } catch { /* 已关闭 */ } }
        if (inst.resizeObserver) inst.resizeObserver.disconnect()
        if (inst.themeObserver) inst.themeObserver.disconnect()
        inst.term.dispose()
    })
    Object.keys(instances).forEach((k) => delete instances[k])
})
defineExpose({ runCommand, attachSession })
</script>

<style scoped>
.terminal-panel { display: flex; flex-direction: column; min-height: 0; height: 100%; }
.term-tabs { display: flex; align-items: stretch; gap: 2px; padding: 2px 4px 4px; flex: none; min-width: 0; border-bottom: 1px solid var(--border); }
.term-tabs-scroll { flex: 1 1 auto; min-width: 0; display: flex; align-items: stretch; gap: 2px; overflow-x: auto; overflow-y: hidden; scrollbar-width: none; }
.term-tabs-scroll::-webkit-scrollbar { height: 0; }
.term-tabs-scroll:hover { scrollbar-width: thin; scrollbar-color: var(--border) transparent; }
.term-tabs-scroll:hover::-webkit-scrollbar { height: 4px; }
.term-tabs-scroll:hover::-webkit-scrollbar-thumb { background: var(--border); border-radius: 2px; }
.term-tabs-scroll:hover::-webkit-scrollbar-track { background: transparent; }
.term-tab { display: flex; align-items: center; gap: 6px; flex: 0 0 auto; white-space: nowrap; max-width: 180px; padding: 3px 8px; border: 1px solid transparent; border-bottom: 2px solid transparent; border-radius: 4px 4px 0 0; background: transparent; cursor: pointer; font-size: 11px; color: var(--muted-foreground); }
.term-tab:hover { background: var(--accent); }
.term-tab.active { color: var(--foreground); border-bottom-color: var(--primary); background: var(--background); }
.term-tab-title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; }
.term-tab-cwd { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--muted-foreground); max-width: 90px; }
.term-tab-close { display: inline-flex; align-items: center; justify-content: center; padding: 0; border: none; background: transparent; color: var(--muted-foreground); cursor: pointer; opacity: 0.5; }
.term-tab-close:hover { color: var(--destructive); opacity: 1; }
.term-tab-add { margin-left: 2px; flex: none; width: 24px; border: 1px dashed var(--border); border-radius: 4px; background: transparent; color: var(--muted-foreground); font-size: 14px; cursor: pointer; }
.term-tab-add:hover { color: var(--primary); border-color: var(--primary); }
.term-tabs-actions { display: flex; align-items: center; flex: none; gap: 4px; white-space: nowrap; }
.term-btn { flex: none; white-space: nowrap; padding: 2px 8px; border: 1px solid var(--border); border-radius: 4px; background: transparent; color: var(--muted-foreground); font-size: 11px; cursor: pointer; }
.term-btn:hover { color: var(--primary); border-color: var(--primary); }
.term-body { position: relative; flex: 1; min-height: 0; border: 1px solid var(--border); border-top: none; border-radius: 0 0 6px 6px; overflow: hidden; }
.term-host { position: absolute; inset: 0; padding: 6px; }
.term-empty { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; gap: 8px; background: var(--background); color: var(--muted-foreground); font-size: 12px; }
.term-retry { padding: 2px 10px; border: 1px solid var(--border); border-radius: 4px; background: transparent; color: var(--primary); font-size: 12px; cursor: pointer; }
</style>
