<template>
    <div class="tool-card kind-knowledge" :class="{ open }">
        <div class="tool-head" role="button" tabindex="0" :aria-expanded="open" @click="toggle" @keydown.enter.prevent="toggle" @keydown.space.prevent="toggle">
            <BookOpen class="tool-icon" :size="14" />
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
            <template v-if="isList">
                <div v-if="rows.length" class="skill-list">
                    <div v-for="item in rows" :key="item.name" class="skill-row">
                        <BookOpen class="skill-icon" :size="12" />
                        <span class="skill-name" :title="item.name">{{ item.name }}</span>
                        <span v-if="item.description" class="skill-desc" :title="item.description">{{ item.description }}</span>
                    </div>
                </div>
                <div v-else class="skill-empty">{{ outputText || '（当前没有启用的技能）' }}</div>
            </template>
            <template v-else>
                <div v-if="path" class="skill-path">{{ path }}</div>
                <div v-if="description" class="skill-desc">{{ description }}</div>
                <div class="tool-section">
                    <div class="tool-label">技能正文</div>
                    <CodeEditor :model-value="bodyText" language="markdown" read-only :height="codeHeight(bodyText)" />
                </div>
            </template>
        </div>
    </div>
</template>

<script setup>
import { computed, ref } from 'vue'
import { BookOpen, ChevronRight, CircleCheck, CircleX, Clock3, TriangleAlert } from 'lucide-vue-next'
import CodeEditor from '../editor/CodeEditor.vue'
import { STATUS_LABEL, codeHeight, firstLine, formatDuration, outputOf, parseArgs, resolveStatus, resultOutput, toolNameOf } from './toolPart'
const props = defineProps({ part: { type: Object, required: true } })
const open = ref(false)
const toggle = () => { open.value = !open.value }
const status = computed(() => resolveStatus(props.part))
const statusText = computed(() => STATUS_LABEL[status.value] || '')
const durationText = computed(() => formatDuration(props.part.durationMs))
const isList = computed(() => toolNameOf(props.part) === 'skill_list')
const skillName = computed(() => String(props.part.skillName || props.part.meta?.skill || 'skill'))
const title = computed(() => (isList.value ? '技能 · 列表' : `技能 · ${skillName.value}`))
const path = computed(() => String(props.part.skillPath || props.part.meta?.path || ''))
const description = computed(() => String(props.part.description || props.part.meta?.description || ''))
const outputText = computed(() => resultOutput(outputOf(props.part)))
const bodyText = computed(() => outputText.value)
const rows = computed(() => {
    if (!isList.value) return []
    const parsed = parseArgs(outputText.value)
    if (!Array.isArray(parsed)) return []
    return parsed.filter((item) => item && typeof item === 'object' && !Array.isArray(item)).map((item) => ({
        name: String(item.name || ''),
        description: String(item.description || ''),
    })).filter((item) => item.name)
})
const summary = computed(() => {
    if (status.value === 'running') return ''
    if (props.part.rejected) return '未获批准，未执行'
    if (isList.value) return rows.value.length ? `${rows.value.length} 个技能` : firstLine(outputText.value)
    if (description.value) return description.value
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
.skill-list { display: flex; flex-direction: column; gap: 2px; }
.skill-row { display: flex; align-items: center; gap: 6px; min-height: 20px; padding: 2px 4px; border-radius: 4px; }
.skill-row:hover { background: var(--accent); }
.skill-icon { flex: none; color: var(--muted-foreground); }
.skill-name { flex: none; max-width: 45%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--foreground); }
.skill-desc { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--muted-foreground); }
.skill-empty { color: var(--muted-foreground); }
.skill-path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--muted-foreground); font-size: 11px; font-family: Consolas, Menlo, 'Courier New', monospace; }
.tool-section { display: flex; flex-direction: column; gap: 4px; }
.tool-label { color: var(--muted-foreground); font-weight: 600; }
</style>