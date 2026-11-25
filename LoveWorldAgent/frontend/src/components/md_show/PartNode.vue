<template>
    <div v-if="kind === 'thinking'" class="thinking-part">
        <button class="thought-toggle" type="button" :class="{ open: thoughtOpen }" :aria-expanded="thoughtOpen" @click="thoughtOpen = !thoughtOpen">
            <BrainCircuit class="thought-icon" :size="12" />
            <span>{{ thoughtTitle }}</span>
            <span v-if="durationText" class="thought-duration">{{ durationText }}</span>
            <ChevronRight class="arrow" :class="{ open: thoughtOpen }" :size="12" />
        </button>
        <div v-if="content" v-show="thoughtOpen" class="thought-body"><MdShow :content="content" :streaming="streaming" /></div>
    </div>
    <MdShow v-else-if="kind === 'markdown'" :content="content" :streaming="streaming" />
    <component :is="componentOf" v-else-if="componentOf" :part="cardPart" @decision="(p, value) => $emit('decision', p, value)" @answer="(p, list) => $emit('answer', p, list)" @open-file="(payload) => $emit('open-file', payload)" />
    <UnsupportedCard v-else-if="kind === 'error'" :part="errorPart" :can-continue="canContinue" @continue="$emit('continue')" />
    <UnsupportedCard v-else :part="unsupportedPart" />
</template>

<script setup>
import { computed, ref } from 'vue'
import { BrainCircuit, ChevronRight } from 'lucide-vue-next'
import MdShow from './MdShow.vue'
import SkillCall from './SkillCall.vue'
import FileEdit from './FileEdit.vue'
import CommandCall from './CommandCall.vue'
import McpCall from './McpCall.vue'
import RuleCall from './RuleCall.vue'
import UpdateList from './UpdateList.vue'
import DiffChangesCard from './DiffChangesCard.vue'
import BuildToolCall from './BuildToolCall.vue'
import SubAgents from './SubAgents.vue'
import ConfirmationCard from './ConfirmationCard.vue'
import QuestionCard from './QuestionCard.vue'
import CompactionCard from './CompactionCard.vue'
import UnsupportedCard from './UnsupportedCard.vue'
import { firstLine, formatDuration, outputOf, resultOutput, toolKindOf, toolNameOf } from './toolPart'
const props = defineProps({ part: { type: Object, required: true }, canContinue: { type: Boolean, default: false } })
defineEmits(['decision', 'answer', 'open-file', 'continue'])
const thoughtOpen = ref(false)
const REGISTRY = {
    terminal: CommandCall,
    diff: DiffChangesCard,
    todo: UpdateList,
    confirmation: ConfirmationCard,
    question: QuestionCard,
    subagent: SubAgents,
    compaction: CompactionCard,
    error: null,
}
const kind = computed(() => {
    const type = String(props.part.type || '')
    if (type === 'thinking') return 'thinking'
    if (type === 'markdown' || type === 'text') return 'markdown'
    return type
})
const content = computed(() => String(props.part.content || ''))
const streaming = computed(() => Boolean(props.part.isStreaming || props.part.streaming))
const durationText = computed(() => formatDuration(props.part.durationMs))
const thoughtTitle = computed(() => {
    if (streaming.value) return '正在思考…'
    const matched = /^\s*\*\*([^*\n]+)\*\*/.exec(content.value)
    const title = matched ? matched[1].trim() : ''
    if (title === '思考过程') return '思考过程'
    return title ? `思考已完成 · ${title}` : '思考已完成'
})
const BUILTIN_TOOL_PREFIXES = ['browser_', 'word_', 'computer_', 'ppt_', 'excel_', 'mobile_']
const toolComponentOf = computed(() => {
    const kind = toolKindOf(props.part)
    const name = toolNameOf(props.part)
    if (kind === 'terminal' || kind === 'command') return CommandCall
    if (name.startsWith('mcp_')) return McpCall
    if (name.startsWith('rule_')) return RuleCall
    if (name.startsWith('skill_')) return SkillCall
    if (name.startsWith('file_')) return FileEdit
    // 子智能体派发（agent_call）已由 goosePart 换成 subagent 卡，这里只兜住直接调用的工具分片
    if (BUILTIN_TOOL_PREFIXES.some((prefix) => name.startsWith(prefix))) return BuildToolCall
    return null
})
const componentOf = computed(() => {
    const type = String(props.part.type || '')
    if (type === 'tool') return toolComponentOf.value || UnsupportedCard
    return REGISTRY[type] || null
})
const toolFallbackPart = computed(() => ({
    type: 'unsupported',
    originalType: 'tool',
    title: `未支持的工具 · ${toolNameOf(props.part)}`,
    message: firstLine(resultOutput(outputOf(props.part))) || '该工具暂无专用卡片',
    original: props.part,
}))
const cardPart = computed(() => (props.part.type === 'tool' && !toolComponentOf.value ? toolFallbackPart.value : props.part))
const unsupportedPart = computed(() => (props.part.type === 'unsupported' ? props.part : { type: 'unsupported', originalType: props.part.type, original: props.part }))
const errorPart = computed(() => ({ type: 'unsupported', originalType: 'error', title: '对话出错', message: content.value || '本轮执行失败', original: props.part }))
</script>

<style scoped>
.thinking-part { width: 100%; }
.thought-toggle { display: inline-flex; align-items: center; gap: 4px; padding: 2px 6px; border: none; border-radius: 4px; background: transparent; color: var(--muted-foreground); font-size: 12px; cursor: pointer; }
.thought-toggle:hover { background: var(--accent); }
.thought-icon { flex: none; color: var(--muted-foreground); }
.thought-duration { flex: none; color: var(--muted-foreground); font-size: 11px; }
.thought-toggle .arrow { transition: transform 0.15s ease; }
.thought-toggle.open .arrow { transform: rotate(90deg); }
.thought-body { width: 100%; max-height: 260px; overflow: auto; margin-top: 4px; padding: 8px 12px; border-left: 2px solid var(--border); background: var(--muted); }
.thought-body :deep(.md-show) { font-size: 12px; color: var(--muted-foreground); }
</style>