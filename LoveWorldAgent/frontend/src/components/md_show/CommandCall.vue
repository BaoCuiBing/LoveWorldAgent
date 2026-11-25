<template>
    <div class="tool-card kind-terminal" :class="{ open }">
        <div class="tool-head" role="button" tabindex="0" :aria-expanded="open" @click="toggle" @keydown.enter.prevent="toggle" @keydown.space.prevent="toggle">
            <Ticket class="tool-icon" :size="14" />
            <span class="tool-title">{{ display }}</span>
            <span v-if="stateText" class="tool-status" :class="status">
                <Clock3 v-if="status === 'running'" :size="14" />
                <CircleX v-else-if="status === 'failed'" :size="14" />
                <TriangleAlert v-else-if="status === 'pending'" :size="14" />
                <CircleCheck v-else :size="14" />
                {{ stateText }}
            </span>
            <span v-if="summaryText" class="tool-summary">{{ summaryText }}</span>
            <span v-if="durationText" class="tool-duration">{{ durationText }}</span>
            <DropdownMenu>
                <DropdownMenuTrigger as-child>
                    <Button class="cmd-more" variant="ghost" size="icon-sm" aria-label="更多操作" @click.stop>
                        <Ellipsis :size="14" />
                    </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" class="w-36">
                    <DropdownMenuItem @select="onMenu('copy')">复制命令</DropdownMenuItem>
                    <DropdownMenuItem @select="onMenu('terminal')">在终端查看</DropdownMenuItem>
                    <DropdownMenuItem @select="onMenu('allow')">加入白名单</DropdownMenuItem>
                </DropdownMenuContent>
            </DropdownMenu>
            <ChevronRight class="arrow" :class="{ open }" :size="12" />
        </div>
        <div v-if="open" class="tool-body">
            <div class="cmd-meta">
                <span class="cmd-mode">{{ modeText }}</span>
                <span v-if="cwd" class="cmd-cwd">{{ cwd }}</span>
                <span v-if="exitCode !== null" class="cmd-exit" :class="{ bad: exitCode !== 0 }">退出码 {{ exitCode }}</span>
                <span v-if="background" class="cmd-badge">后台运行</span>
            </div>
            <div v-if="command" class="cmd-line"><span class="cmd-prompt">$</span>{{ command }}</div>
            <CodeEditor v-if="outputText" :model-value="outputText" language="plaintext" read-only :height="codeHeight(outputText)" />
            <div v-else-if="loadingResult" class="cmd-loading">
                <span class="cmd-shimmer" />
                <span class="cmd-shimmer short" />
                <span class="cmd-loading-text">加载中…</span>
            </div>
            <div v-else class="cmd-none">（无输出）</div>
            <div v-if="allowArmed" class="cmd-confirm">
                <span>该命令将免审执行，确认加入白名单？</span>
                <Button size="sm" variant="outline" @click="allowArmed = false">取消</Button>
                <Button size="sm" :disabled="countdown > 0" @click="confirmAllow">{{ countdown > 0 ? `确认 (${countdown}s)` : '确认加入' }}</Button>
            </div>
        </div>
    </div>
</template>

<script setup>
import { computed, onBeforeUnmount, ref } from 'vue'
import { ChevronRight, CircleCheck, CircleX, Clock3, Ellipsis, Ticket, TriangleAlert } from 'lucide-vue-next'
import { Button } from '@/components/shadcn_ui/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/shadcn_ui/ui/dropdown-menu'
import { toast } from '@/components/shadcn_ui/lib/useToast'
import CodeEditor from '../editor/CodeEditor.vue'
import { permissionAllow, permissionCheck } from '../../api/request_data'
import { codeHeight, formatDuration, parseArgs, resolveStatus, resultOutput, runningWithoutResult } from './toolPart'
const props = defineProps({ part: { type: Object, required: true } })
const open = ref(false)
const toggle = () => { open.value = !open.value }
const durationText = computed(() => formatDuration(props.part.durationMs))
const args = computed(() => {
    const parsed = parseArgs(props.part.arguments)
    return parsed && typeof parsed === 'object' ? parsed : {}
})
const command = computed(() => String(args.value.command || args.value.text || props.part.meta?.command || ''))
const cwd = computed(() => String(props.part.meta?.cwd || args.value.cwd || ''))
const background = computed(() => Boolean(props.part.meta?.background || args.value.background))
const exitCode = computed(() => {
    const raw = props.part.meta?.exitCode
    return raw === undefined || raw === null || raw === '' ? null : Number(raw)
})
const status = computed(() => resolveStatus(props.part))
const loadingResult = computed(() => runningWithoutResult(props.part))
const TERMINAL_TOOL_LABELS = { terminal_open: '打开终端', terminal_send: '终端输入', terminal_read: '读取终端', terminal_list: '终端列表', command_run: '执行命令' }
const stateText = computed(() => {
    const tool = String(props.part.toolName || '')
    if (status.value === 'running') return '执行中'
    if (status.value === 'failed') {
        if (tool === 'terminal_open') return '打开失败'
        if (tool === 'terminal_read') return '读取失败'
        return background.value ? '启动失败' : '命令失败'
    }
    if (background.value) return '后台运行中'
    if (exitCode.value !== null && exitCode.value !== 0) return '命令失败'
    return command.value ? '命令已执行' : '已执行'
})
const outputText = computed(() => resultOutput(props.part.output))
const summaryText = computed(() => String(props.part.output || '').split('\n')[0].slice(0, 90) || (command.value ? '(无输出)' : ''))
const display = computed(() => {
    if (command.value) return `$ ${command.value}`
    const tool = String(props.part.toolName || '')
    const base = TERMINAL_TOOL_LABELS[tool] || tool || '终端操作'
    const suffix = args.value.cwd || args.value.session_id || ''
    return suffix ? `${base} · ${suffix}` : base
})
const modeText = computed(() => (props.part.runMode === 'sandbox' ? '沙箱内运行' : (props.part.runMode === 'terminal' || props.part.meta?.sessionId) ? '终端会话' : '直接执行'))
const allowArmed = ref(false)
const countdown = ref(3)
let timer = null
const onMenu = (value) => {
    if (value === 'copy') copyCommand()
    else if (value === 'terminal') checkAndRun()
    else if (value === 'allow') armAllow()
}
const copyCommand = async () => {
    try { await navigator.clipboard.writeText(command.value); toast.success('已复制命令') } catch { toast.error('复制失败') }
}
const runInTerminal = () => window.dispatchEvent(new CustomEvent('lwa:terminal-run', { detail: { command: command.value, cwd: cwd.value } }))
const checkAndRun = async () => {
    if (!command.value) { toast.warning('没有可执行的命令'); return }
    try {
        const data = await permissionCheck(command.value)
        if (data && data.allowed) { runInTerminal(); return }
        toast.warning((data && data.reason) || '该命令需要先在对话中获批准')
    } catch (err) {
        toast.error(`权限检查失败：${err.message}`)
    }
}
const armAllow = () => {
    if (!command.value) { toast.warning('没有可加入白名单的命令'); return }
    allowArmed.value = true
    countdown.value = 3
    if (timer) clearInterval(timer)
    timer = setInterval(() => {
        if (countdown.value > 0) countdown.value -= 1
        else { clearInterval(timer); timer = null }
    }, 1000)
}
const confirmAllow = async () => {
    if (countdown.value > 0) return
    try {
        await permissionAllow(command.value)
        toast.success('已加入白名单')
        allowArmed.value = false
    } catch (err) {
        toast.error(`加入白名单失败：${err.message}`)
    }
}
onBeforeUnmount(() => { if (timer) clearInterval(timer) })
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
.cmd-more { flex: none; width: 22px; height: 22px; opacity: 0.65; transition: opacity 0.15s ease; }
.cmd-more:hover { opacity: 1; }
.cmd-meta { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; color: var(--muted-foreground); font-size: 11px; }
.cmd-mode { padding: 1px 6px; border-radius: 4px; background: var(--muted); }
.cmd-cwd { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 45%; }
.cmd-exit { color: #22c55e; font-family: Consolas, Menlo, 'Courier New', monospace; }
.cmd-exit.bad { color: var(--destructive); }
.cmd-badge { padding: 1px 6px; border-radius: 4px; background: color-mix(in srgb, var(--primary) 12%, transparent); color: var(--primary); }
.cmd-line { font-family: Consolas, Menlo, 'Courier New', monospace; color: var(--foreground); white-space: pre-wrap; word-break: break-all; }
.cmd-prompt { margin-right: 6px; color: #22c55e; font-weight: 600; }
.cmd-out { margin: 0; padding: 8px; border-radius: 4px; background: var(--muted); color: var(--foreground); font-size: 12px; line-height: 1.5; font-family: Consolas, Menlo, 'Courier New', monospace; white-space: pre-wrap; word-break: break-all; max-height: 280px; overflow: auto; }
.cmd-none { color: var(--muted-foreground); font-size: 12px; }
.cmd-loading { display: flex; flex-direction: column; gap: 6px; padding: 8px; border-radius: 4px; background: var(--muted); }
.cmd-shimmer { display: block; width: 72%; height: 8px; border-radius: 4px; background: linear-gradient(90deg, var(--muted) 25%, var(--accent) 50%, var(--muted) 75%); background-size: 200% 100%; animation: cb-shimmer 1.2s ease-in-out infinite; }
.cmd-shimmer.short { width: 46%; }
.cmd-loading-text { color: var(--muted-foreground); font-size: 11px; }
.cmd-confirm { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border: 1px solid color-mix(in srgb, #f59e0b 35%, transparent); border-radius: 6px; background: color-mix(in srgb, #f59e0b 12%, transparent); color: var(--muted-foreground); font-size: 11px; }
.cmd-confirm span { flex: 1; min-width: 0; }
@keyframes cb-shimmer { 0% { background-position: 200% 0; } 100% { background-position: -200% 0; } }
</style>
