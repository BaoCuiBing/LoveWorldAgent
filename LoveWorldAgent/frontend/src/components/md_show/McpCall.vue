<template>
    <div class="tool-card kind-mcp" :class="{ open }">
        <div class="tool-head" role="button" tabindex="0" :aria-expanded="open" @click="toggle" @keydown.enter.prevent="toggle" @keydown.space.prevent="toggle">
            <Cable class="tool-icon" :size="14" />
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
import { Cable, ChevronRight, CircleCheck, CircleX, Clock3, TriangleAlert } from 'lucide-vue-next'
import CodeEditor from '../editor/CodeEditor.vue'
import { STATUS_LABEL, argsOf, codeHeight, firstLine, formatDuration, formatValue, outputOf, resolveStatus, resultOutput, toolNameOf } from './toolPart'
const props = defineProps({ part: { type: Object, required: true } })
/** mcp_{服务}__{工具}：服务名与工具名中的非安全字符已被后端替换为下划线 */
const splitName = (name) => {
    const raw = String(name || '').replace(/^mcp_/, '')
    const index = raw.indexOf('__')
    return index < 0 ? { server: raw, tool: '' } : { server: raw.slice(0, index), tool: raw.slice(index + 2) }
}
const open = ref(false)
const toggle = () => { open.value = !open.value }
const status = computed(() => resolveStatus(props.part))
const statusText = computed(() => STATUS_LABEL[status.value] || '')
const durationText = computed(() => formatDuration(props.part.durationMs))
const parsedName = computed(() => splitName(toolNameOf(props.part)))
const title = computed(() => ['MCP', parsedName.value.server, parsedName.value.tool].filter(Boolean).join(' · '))
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
.tool-card.kind-mcp { border-style: dashed; }
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
