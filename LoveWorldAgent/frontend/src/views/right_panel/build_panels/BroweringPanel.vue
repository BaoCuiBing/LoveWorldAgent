<template>
    <div class="bp-root">
        <div v-if="!embedded" class="bp-head" @click="collapsed = !collapsed">
            <Globe class="bp-head-icon" :size="14" />
            <span class="bp-head-title">已操作浏览器</span>
            <span class="bp-head-count">{{ items.length }}</span>
            <ChevronDown class="bp-head-arrow" :class="{ collapsed }" :size="12" />
        </div>
        <div v-show="embedded || !collapsed" class="bp-list">
            <div v-if="!items.length" class="bp-empty">
                <div class="bp-empty-icon"><Globe :size="20" /></div>
                <div class="bp-empty-title">暂无浏览器操作</div>
                <div class="bp-empty-desc">智能体调用浏览器工具后会在此列出步骤</div>
            </div>
            <div v-for="(item, index) in items" :key="item.seq" class="bp-item">
                <div class="bp-line clickable" @click="toggleExpand(item.seq)">
                    <ChevronRight class="bp-line-arrow" :class="{ open: expanded[item.seq] }" :size="12" />
                    <FileText class="bp-line-icon" :size="13" />
                    <span class="bp-line-text">{{ item.text }}</span>
                    <span v-if="item.args && item.args.url" class="bp-line-url">{{ item.args.url }}</span>
                    <span v-if="!item.ok" class="bp-line-err">失败</span>
                </div>
                <div v-if="expanded[item.seq]" class="bp-detail">
                    <div class="bp-detail-row"><span class="bp-detail-key">工具</span><span class="bp-detail-val">{{ item.tool }}</span></div>
                    <div class="bp-detail-row"><span class="bp-detail-key">参数</span><pre class="bp-detail-pre">{{ formatPayload(item.args) }}</pre></div>
                    <div v-if="item.result" class="bp-detail-row"><span class="bp-detail-key">返回</span><pre class="bp-detail-pre">{{ item.result }}</pre></div>
                </div>
                <div v-if="item.screenshot" class="bp-shot">
                    <img v-if="shots[item.screenshot]" :src="shots[item.screenshot]" alt="浏览器截图" class="bp-shot-img" @click="previewSrc = shots[item.screenshot]" />
                    <div v-else class="bp-shot-loading">截图加载中…</div>
                </div>
                <div v-if="index < items.length - 1" class="bp-connector"></div>
            </div>
        </div>
        <div class="bp-foot">
            <Input v-model="url" class="bp-foot-input" placeholder="输入网址，回车在浏览器窗口打开" @keyup.enter="openWindow" />
            <div class="bp-foot-actions">
                <Button size="sm" variant="default" :disabled="busy" @click="openWindow">
                    <LoaderCircle v-if="busy" class="animate-spin" :size="14" />
                    <span>打开浏览器窗口</span>
                </Button>
                <span v-if="busy" class="bp-busy">启动中…</span>
                <Button v-if="status.running" size="sm" variant="ghost" @click="closeWindow">关闭</Button>
            </div>
        </div>
        <Dialog :open="Boolean(previewSrc)" @update:open="(value) => { if (!value) previewSrc = '' }">
            <DialogContent class="bp-preview max-w-[90vw]">
                <DialogTitle class="sr-only">截图预览</DialogTitle>
                <img v-if="previewSrc" :src="previewSrc" alt="浏览器截图预览" class="bp-preview-img" />
            </DialogContent>
        </Dialog>
    </div>
</template>

<script setup>
import { onBeforeUnmount, reactive, ref, watch } from 'vue'
import { ChevronDown, ChevronRight, FileText, Globe, LoaderCircle } from 'lucide-vue-next'
import { Button } from '../../../components/shadcn_ui/ui/button'
import { Dialog, DialogContent, DialogTitle } from '../../../components/shadcn_ui/ui/dialog'
import { Input } from '../../../components/shadcn_ui/ui/input'
import { toast } from '../../../components/shadcn_ui/lib/useToast'
import { browserScreenshot, browserState, browserSteps, startBrowser, stopBrowser } from '../../../api/request_data'
const props = defineProps({
    embedded: { type: Boolean, default: false },
})
const emit = defineEmits(['meta'])
const items = ref([])
const shots = reactive({})
const expanded = reactive({})
const collapsed = ref(false)
const toggleExpand = (seq) => { expanded[seq] = !expanded[seq] }
const formatPayload = (value) => {
    if (!value || (typeof value === 'object' && !Object.keys(value).length)) return '{}'
    try { return JSON.stringify(value, null, 2) } catch { return String(value) }
}
const busy = ref(false)
const url = ref('')
const previewSrc = ref('')
const status = reactive({ running: false, pipe: false, starting: false, visible: false, error: '', title: '', address: '' })
let after = 0
let timer = 0
const emitMeta = () => emit('meta', { count: items.value.length, running: status.running })
watch(() => [items.value.length, status.running], emitMeta, { immediate: true })
const syncSteps = async () => {
    try {
        const data = await browserSteps(after)
        const list = (data && data.items) || []
        if (list.length) {
            items.value = [...items.value, ...list].slice(-200)
            after = Math.max(after, ...list.map((item) => Number(item.seq) || 0))
            list.filter((item) => item.screenshot && !shots[item.screenshot]).forEach(loadShot)
        }
    } catch { /* 步骤轮询失败静默 */ }
}
const loadShot = async (item) => {
    try {
        const data = await browserScreenshot(item.screenshot)
        shots[item.screenshot] = `data:${data.mime};base64,${data.base64}`
    } catch { /* 截图读取失败则不展示 */ }
}
const syncState = async () => {
    try {
        const data = await browserState()
        status.running = Boolean(data.running)
        status.pipe = Boolean(data.pipe)
        status.starting = Boolean(data.starting)
        status.visible = Boolean(data.visible)
        status.error = String(data.error || '')
        status.title = String(data.title || '')
        if (data.address) {
            status.address = String(data.address)
            if (!url.value) url.value = String(data.address)
        }
    } catch { /* 状态轮询失败静默 */ }
}
const waitReady = async () => {
    const deadline = Date.now() + 15000
    let networkError = ''
    while (Date.now() < deadline) {
        let data = null
        try {
            data = await browserState()
        } catch (err) {
            networkError = String(err.message || '后端未响应')
        }
        if (data) {
            status.running = Boolean(data.running)
            status.pipe = Boolean(data.pipe)
            status.starting = Boolean(data.starting)
            status.error = String(data.error || '')
            if (status.error) throw new Error(`启动失败：${status.error}`)
            if (status.running && status.pipe) return
        }
        await new Promise((resolve) => window.setTimeout(resolve, 300))
    }
    throw new Error(networkError ? `启动超时（15s）：后端未响应（${networkError}）` : '启动超时（15s）：浏览器窗口或管道未就绪')
}
const openWindow = async () => {
    if (busy.value) return
    const text = String(url.value || '').trim()
    busy.value = true
    try {
        const res = await startBrowser({ x: 0, y: 0, w: 900, h: 700 }, text)
        if (res && res.ok === false) throw new Error(res.error || '启动请求被拒绝')
        await waitReady()
        await syncState()
        startTimer()
    } catch (err) {
        toast.error(`打开浏览器失败：${err.message}`)
    } finally {
        busy.value = false
    }
}
const closeWindow = async () => {
    try {
        await stopBrowser()
    } catch (err) {
        toast.error(`关闭浏览器失败：${err.message}`)
    }
    stopTimer()
    status.running = false
}
const startTimer = () => {
    if (timer) return
    syncSteps()
    syncState()
    timer = window.setInterval(() => {
        syncSteps()
        syncState()
    }, 1200)
}
const stopTimer = () => {
    if (!timer) return
    window.clearInterval(timer)
    timer = 0
}
startTimer()
onBeforeUnmount(stopTimer)
</script>

<style scoped>
.bp-root { display: flex; flex-direction: column; flex: 1; min-height: 0; }
.bp-head { display: flex; flex: none; align-items: center; gap: 6px; padding: 8px 10px; cursor: pointer; color: var(--foreground); font-size: 12px; }
.bp-head-icon { color: var(--primary); }
.bp-head-title { font-weight: 600; }
.bp-head-count { margin-left: auto; padding: 0 6px; border-radius: 8px; background: var(--accent); color: var(--muted-foreground); font-size: 11px; }
.bp-head-arrow { transition: transform 0.2s; color: var(--muted-foreground); }
.bp-head-arrow.collapsed { transform: rotate(-90deg); }
.bp-list { display: flex; flex-direction: column; flex: 1; min-height: 0; overflow-y: auto; padding: 0 10px 8px; }
.bp-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; flex: 1; min-height: 120px; gap: 4px; padding: 18px 8px; text-align: center; }
.bp-empty-icon { display: inline-flex; align-items: center; justify-content: center; width: 42px; height: 42px; border-radius: 10px; background: var(--accent); color: var(--muted-foreground); }
.bp-empty-title { font-size: 12px; font-weight: 600; color: var(--foreground); }
.bp-empty-desc { font-size: 11px; line-height: 1.6; color: var(--muted-foreground); }
.bp-item { position: relative; padding: 2px 0 6px 0; }
.bp-line { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--foreground); }
.bp-line.clickable { cursor: pointer; padding: 2px 4px; border-radius: 4px; }
.bp-line.clickable:hover { background: var(--accent); }
.bp-line-arrow { flex: none; color: var(--muted-foreground); transition: transform 0.15s ease; }
.bp-line-arrow.open { transform: rotate(90deg); }
.bp-line-icon { flex: none; color: var(--muted-foreground); }
.bp-line-text { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.bp-line-url { flex: none; max-width: 46%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--muted-foreground); font-size: 11px; }
.bp-line-err { flex: none; color: var(--destructive); font-size: 11px; }
.bp-detail { margin: 4px 0 2px 18px; padding: 2px 0 2px 8px; border-left: 2px solid var(--border); display: flex; flex-direction: column; gap: 4px; font-size: 11px; color: var(--muted-foreground); }
.bp-detail-row { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.bp-detail-key { font-size: 10px; color: var(--muted-foreground); }
.bp-detail-val, .bp-detail-pre { margin: 0; font-family: Consolas, monospace; font-size: 11px; line-height: 1.45; color: var(--foreground); white-space: pre-wrap; word-break: break-word; max-height: 180px; overflow-y: auto; }
.bp-connector { margin: 5px 0 0 6px; width: 1px; height: 14px; background: var(--border); }
.bp-shot { margin: 6px 0 0 20px; max-width: 220px; }
.bp-shot-img { width: 100%; border: 1px solid var(--border); border-radius: 6px; cursor: zoom-in; }
.bp-shot-loading { font-size: 11px; color: var(--muted-foreground); }
.bp-foot { display: flex; flex: none; flex-direction: row; align-items: center; gap: 6px; padding: 8px 10px; border-top: 1px solid var(--border); }
.bp-foot-input { flex: 1; min-width: 0; }
.bp-foot-actions { display: flex; align-items: center; gap: 6px; flex: none; }
.bp-busy { font-size: 11px; color: var(--muted-foreground); }
.bp-preview { width: auto; }
.bp-preview-img { display: block; max-width: 100%; max-height: calc(100vh - 160px); margin: 0 auto; border-radius: 4px; }
</style>
