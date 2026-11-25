<template>
    <div class="tool-card kind-knowledge" :class="{ open }">
        <div class="tool-head" role="button" tabindex="0" :aria-expanded="open" @click="toggle" @keydown.enter.prevent="toggle" @keydown.space.prevent="toggle">
            <FileText class="tool-icon" :size="14" />
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
            <div v-if="isList" class="rule-list">
                <div v-for="item in rows" :key="item.name" class="rule-row">
                    <FileText class="rule-icon" :size="12" />
                    <span class="rule-name" :title="item.name">{{ item.name }}</span>
                    <span v-if="item.trigger" class="rule-tag">{{ item.trigger }}</span>
                    <span v-if="item.description" class="rule-desc" :title="item.description">{{ item.description }}</span>
                </div>
                <div v-if="!rows.length" class="rule-empty">{{ outputText || '（当前没有任何规则）' }}</div>
            </div>
            <div v-else-if="isRead && status !== 'failed'" class="tool-section">
                <div class="tool-label">规则正文</div>
                <CodeEditor :model-value="bodyText" language="markdown" read-only :height="codeHeight(bodyText)" />
            </div>
            <div v-else class="tool-section">

                <div class="rule-result" :class="{ bad: status === 'failed' }">{{ outputText || '（无输出）' }}</div>
            </div>
        </div>
    </div>
</template>

<script setup>
import { computed, ref } from 'vue'
import { ChevronRight, CircleCheck, CircleX, Clock3, FileText, TriangleAlert } from 'lucide-vue-next'
import CodeEditor from '../editor/CodeEditor.vue'
import { STATUS_LABEL, codeHeight, firstLine, formatDuration, outputOf, parseArgs, resolveStatus, resultOutput, toolNameOf } from './toolPart'
const TITLES = { rule_list: '规则 · 列表', rule_read: '规则 · 读取' }
const TRIGGERS = { always_on: '始终生效', model_decision: '智能生效' }
const props = defineProps({ part: { type: Object, required: true } })
const open = ref(false)
const toggle = () => { open.value = !open.value }
const name = computed(() => toolNameOf(props.part))
const title = computed(() => TITLES[name.value] || '规则')
const status = computed(() => resolveStatus(props.part))
const statusText = computed(() => STATUS_LABEL[status.value] || '')
const durationText = computed(() => formatDuration(props.part.durationMs))
const outputText = computed(() => resultOutput(outputOf(props.part)))
const bodyText = computed(() => outputText.value)
const isList = computed(() => name.value === 'rule_list')
const isRead = computed(() => name.value === 'rule_read')
const rows = computed(() => {
    if (!isList.value) return []
    const parsed = parseArgs(outputText.value)
    if (!Array.isArray(parsed)) return []
    return parsed.filter((item) => item && typeof item === 'object' && !Array.isArray(item)).map((item) => ({
        name: String(item.name || ''),
        trigger: TRIGGERS[String(item.trigger || '')] || '',
        description: String(item.description || item.preview || ''),
    })).filter((item) => item.name)
})
const summary = computed(() => {
    if (status.value === 'running') return ''
    if (status.value === 'failed') return firstLine(outputText.value) || '执行失败'
    if (isList.value) return rows.value.length ? `${rows.value.length} 条规则` : firstLine(outputText.value)
    return firstLine(outputText.value)
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
.rule-list { display: flex; flex-direction: column; gap: 2px; }
.rule-row { display: flex; align-items: center; gap: 6px; min-height: 20px; padding: 2px 4px; border-radius: 4px; }
.rule-row:hover { background: var(--accent); }
.rule-icon { flex: none; color: var(--muted-foreground); }
.rule-name { flex: none; max-width: 45%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--foreground); }
.rule-tag { flex: none; padding: 0 5px; border-radius: 8px; background: var(--muted); color: var(--muted-foreground); font-size: 10px; line-height: 15px; }
.rule-desc { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--muted-foreground); }
.rule-empty { padding: 2px 4px; color: var(--muted-foreground); }
.rule-path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--muted-foreground); font-size: 11px; font-family: Consolas, Menlo, 'Courier New', monospace; }
.rule-result { color: var(--foreground); line-height: 1.6; white-space: pre-wrap; word-break: break-word; }
.rule-result.bad { color: var(--destructive); }
.tool-section { display: flex; flex-direction: column; gap: 4px; }
.tool-label { color: var(--muted-foreground); font-weight: 600; }
</style>
