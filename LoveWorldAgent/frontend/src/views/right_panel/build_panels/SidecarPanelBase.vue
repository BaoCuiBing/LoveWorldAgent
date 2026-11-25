<template>
    <div class="sp-root">
        <div class="sp-list">
            <div v-if="!items.length" class="sp-empty">
                <div class="sp-empty-icon"><component :is="icon" :size="18" /></div>
                <div class="sp-empty-title">{{ emptyTitle }}</div>
                <div class="sp-empty-desc">{{ emptyDesc }}</div>
            </div>
            <div v-for="(item, index) in items" :key="item.seq" class="sp-item">
                <div class="sp-line" @click="toggleExpand(item.seq)">
                    <ChevronRight class="sp-line-arrow" :class="{ open: expanded[item.seq] }" :size="12" />
                    <component :is="item.ok ? CheckCircle2 : AlertCircle" class="sp-line-icon" :class="item.ok ? 'ok' : 'err'" :size="13" />
                    <span class="sp-line-text" :title="item.text">{{ item.text }}</span>
                    <span v-if="argHint(item)" class="sp-line-arg">{{ argHint(item) }}</span>
                    <span v-if="!item.ok" class="sp-line-err" :title="item.error">失败</span>
                </div>
                <div v-if="expanded[item.seq]" class="sp-detail">
                    <div class="sp-detail-row"><span class="sp-detail-key">工具</span><span class="sp-detail-val">{{ item.tool }}</span></div>
                    <div class="sp-detail-row"><span class="sp-detail-key">参数</span><pre class="sp-detail-pre">{{ formatPayload(item.args) }}</pre></div>
                    <div v-if="item.result" class="sp-detail-row"><span class="sp-detail-key">返回</span><pre class="sp-detail-pre">{{ item.result }}</pre></div>
                </div>
                <div v-if="fetchScreenshot && item.screenshot && !shotFailed[item.screenshot]" class="sp-shot">
                    <img v-if="shots[item.screenshot]" :src="shots[item.screenshot]" :alt="shotAlt" class="sp-shot-img" @click="previewSrc = shots[item.screenshot]" />
                    <div v-else class="sp-shot-loading">截图加载中…</div>
                </div>
                <div v-if="index < items.length - 1" class="sp-connector"></div>
            </div>
        </div>
        <div class="sp-foot">
            <span class="sp-status" :class="{ on: state.running }">
                <span class="sp-dot" />
                <span>{{ state.running ? `运行中 (PID ${state.pid || '-'})` : '未启动（工具调用时自动拉起）' }}</span>
            </span>
            <div class="sp-badges">
                <slot name="badges" />
                <Button v-if="state.running" size="sm" variant="ghost" :disabled="stopping" @click="onStop">停止服务</Button>
            </div>
        </div>
        <Dialog :open="Boolean(previewSrc)" @update:open="(value) => { if (!value) previewSrc = '' }">
            <DialogContent class="sp-preview max-w-[90vw]">
                <DialogTitle class="sr-only">截图预览</DialogTitle>
                <DialogDescription class="sr-only">操作截图大图预览</DialogDescription>
                <img v-if="previewSrc" :src="previewSrc" :alt="`${shotAlt}预览`" class="sp-preview-img" />
            </DialogContent>
        </Dialog>
    </div>
</template>

<script setup>
import { onBeforeUnmount, reactive, ref, watch } from 'vue'
import { AlertCircle, CheckCircle2, ChevronRight } from 'lucide-vue-next'
import { Button } from '../../../components/shadcn_ui/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '../../../components/shadcn_ui/ui/dialog'
import { toast } from '../../../components/shadcn_ui/lib/useToast'
const props = defineProps({
    icon: { type: [Object, Function], required: true },
    emptyTitle: { type: String, required: true },
    emptyDesc: { type: String, required: true },
    fetchState: { type: Function, required: true },
    fetchSteps: { type: Function, required: true },
    stopSidecar: { type: Function, required: true },
    fetchScreenshot: { type: Function, default: null },
    shotAlt: { type: String, default: '操作截图' },
})
const emit = defineEmits(['meta'])
const items = ref([])
const expanded = reactive({})
const shots = reactive({})
const shotFailed = reactive({})
const previewSrc = ref('')
const stopping = ref(false)
const state = reactive({ running: false, pid: 0, pipe: false })
let after = 0
let timer = 0
let syncing = false
watch(() => [items.value.length, state.running], () => emit('meta', { count: items.value.length, running: state.running }), { immediate: true })
const toggleExpand = (seq) => { expanded[seq] = !expanded[seq] }
const formatPayload = (value) => {
    if (!value || (typeof value === 'object' && !Object.keys(value).length)) return '{}'
    try { return JSON.stringify(value, null, 2) } catch { return String(value) }
}
const argHint = (item) => {
    const a = (item && item.args) || {}
    if (a.app) return String(a.app)
    if (a.device) return String(a.device)
    if (a.path) return String(a.path).split(/[\\/]/).pop() || ''
    if (a.pid) return `pid:${a.pid}`
    return ''
}
const loadShot = async (path) => {
    if (!props.fetchScreenshot || !path || shots[path] !== undefined || shotFailed[path]) return
    shots[path] = ''
    try {
        const data = await props.fetchScreenshot(path)
        if (data && data.base64) shots[path] = `data:${data.mime || 'image/png'};base64,${data.base64}`
        else {
            delete shots[path]
            shotFailed[path] = true
        }
    } catch {
        delete shots[path]
        shotFailed[path] = true
    }
}
const pruneShots = () => {
    const active = new Set(items.value.map((entry) => entry.screenshot).filter(Boolean))
    Object.keys(shots).forEach((key) => { if (!active.has(key)) delete shots[key] })
    Object.keys(shotFailed).forEach((key) => { if (!active.has(key)) delete shotFailed[key] })
}
const sync = async () => {
    if (syncing) return
    syncing = true
    try {
        const [st, stepRes] = await Promise.all([props.fetchState(), props.fetchSteps(after)])
        if (st) {
            state.running = Boolean(st.running)
            state.pid = Number(st.pid || 0)
            state.pipe = Boolean(st.pipe)
        }
        const rawList = (stepRes && stepRes.items) || []
        const existingSeqs = new Set(items.value.map((entry) => entry.seq))
        const list = []
        for (const entry of rawList) {
            if (!existingSeqs.has(entry.seq)) {
                existingSeqs.add(entry.seq)
                list.push(entry)
            }
        }
        if (list.length) {
            items.value = [...items.value, ...list].slice(-200)
            after = Math.max(after, ...list.map((entry) => Number(entry.seq) || 0))
            if (props.fetchScreenshot) {
                pruneShots()
                list.filter((entry) => entry.screenshot).forEach((entry) => loadShot(entry.screenshot))
            }
        } else if (rawList.length) {
            after = Math.max(after, ...rawList.map((entry) => Number(entry.seq) || 0))
        }
    } catch { /* 轮询失败静默 */ } finally {
        syncing = false
    }
}
const onStop = async () => {
    if (stopping.value) return
    stopping.value = true
    try {
        const res = await props.stopSidecar()
        state.running = Boolean(res && res.running)
        state.pid = Number((res && res.pid) || 0)
        toast.info('已停止服务进程')
    } catch (err) {
        toast.error(`停止失败：${err.message}`)
    } finally {
        stopping.value = false
    }
}
sync()
timer = window.setInterval(sync, 1200)
onBeforeUnmount(() => {
    if (timer) window.clearInterval(timer)
})
</script>

<style scoped>
.sp-root { display: flex; flex-direction: column; flex: 1; min-height: 0; }
.sp-list { display: flex; flex-direction: column; flex: 1; min-height: 0; overflow-y: auto; padding: 4px 10px 8px; }
.sp-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; flex: 1; min-height: 110px; gap: 4px; padding: 16px 8px; text-align: center; }
.sp-empty-icon { display: inline-flex; align-items: center; justify-content: center; width: 40px; height: 40px; border-radius: 10px; background: var(--accent); color: var(--muted-foreground); }
.sp-empty-title { font-size: 12px; font-weight: 600; color: var(--foreground); }
.sp-empty-desc { font-size: 11px; line-height: 1.5; color: var(--muted-foreground); }
.sp-item { position: relative; padding: 2px 0 4px; }
.sp-line { display: flex; align-items: center; gap: 6px; padding: 2px 4px; border-radius: 4px; font-size: 12px; color: var(--foreground); cursor: pointer; }
.sp-line:hover { background: var(--accent); }
.sp-line-arrow { flex: none; color: var(--muted-foreground); transition: transform 0.15s ease; }
.sp-line-arrow.open { transform: rotate(90deg); }
.sp-line-icon { flex: none; }
.sp-line-icon.ok { color: #67c23a; }
.sp-line-icon.err { color: var(--destructive); }
.sp-line-text { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.sp-line-arg { flex: none; max-width: 42%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--muted-foreground); font-size: 11px; }
.sp-line-err { flex: none; color: var(--destructive); font-size: 11px; }
.sp-detail { margin: 4px 0 2px 18px; padding: 2px 0 2px 8px; border-left: 2px solid var(--border); display: flex; flex-direction: column; gap: 4px; font-size: 11px; color: var(--muted-foreground); }
.sp-detail-row { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.sp-detail-key { font-size: 10px; color: var(--muted-foreground); }
.sp-detail-val, .sp-detail-pre { margin: 0; font-family: Consolas, monospace; font-size: 11px; line-height: 1.45; color: var(--foreground); white-space: pre-wrap; word-break: break-word; max-height: 180px; overflow-y: auto; }
.sp-connector { margin: 4px 0 0 10px; width: 1px; height: 12px; background: var(--border); }
.sp-foot { display: flex; flex: none; align-items: center; justify-content: space-between; gap: 8px; padding: 8px 10px; border-top: 1px solid var(--border); }
.sp-status { display: inline-flex; align-items: center; gap: 6px; font-size: 11px; color: var(--muted-foreground); min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.sp-status.on { color: var(--foreground); }
.sp-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--muted-foreground); flex: none; }
.sp-status.on .sp-dot { background: #67c23a; }
.sp-badges { display: inline-flex; align-items: center; gap: 6px; flex: none; }
.sp-shot { margin: 6px 0 0 20px; max-width: 220px; }
.sp-shot-img { width: 100%; border: 1px solid var(--border); border-radius: 6px; cursor: zoom-in; }
.sp-shot-loading { font-size: 11px; color: var(--muted-foreground); }
.sp-preview { width: auto; }
.sp-preview-img { display: block; max-width: 100%; max-height: calc(100vh - 160px); margin: 0 auto; border-radius: 4px; }
</style>
