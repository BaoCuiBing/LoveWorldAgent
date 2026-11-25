<template>
    <div class="ai-message">
        <div class="ai-head">
            <span class="agent-avatar"><img class="agent-avatar-img" :src="dragon" alt="Agent"></span>
            <span class="agent-name">Agent</span>
            <span class="agent-detail">
                <span v-if="active" class="streaming-dot" />
                {{ statusLabel }}
                <span v-if="active" class="animated-ellipsis" />
            </span>
        </div>
        <template v-for="(item, index) in renderItems" :key="item.partKey || index">
            <PartNode :part="item" :can-continue="canContinue" @decision="onDecision" @answer="onAnswer" @open-file="(payload) => $emit('open-file', payload)" @continue="$emit('continue')" />
        </template>
        <div v-if="!parts.length && active" class="thinking-hint">正在思考…</div>
        <div v-if="failure" class="failure-card" role="note">
            <TriangleAlert :size="14" />
            <span>{{ failure.message || '本轮任务失败' }}</span>
        </div>
        <div v-if="finished" class="ai-actions">
            <div class="action-left">
                <Button v-if="!assistant.isCompaction" class="mini-btn" variant="ghost" size="icon-sm" aria-label="复制" @click="copy">
                    <Copy :size="14" />
                </Button>
                <Button v-if="finished" class="mini-btn" variant="ghost" size="icon-sm" aria-label="重试本轮" @click="$emit('retry')">
                    <RotateCw :size="14" />
                </Button>
                <Button v-if="finished" class="mini-btn" variant="ghost" size="icon-sm" aria-label="删除本轮" @click="$emit('delete')">
                    <Trash2 :size="14" />
                </Button>
            </div>
            <span v-if="usageText" class="footer-details">消耗 token：{{ usageText }}</span>
        </div>
    </div>
</template>

<script setup>
import { computed } from 'vue'
import { Copy, RotateCw, Trash2, TriangleAlert } from 'lucide-vue-next'
import dragon from '../../assets/dragon.png'
import { Button } from '@/components/shadcn_ui/ui/button'
import { toast } from '@/components/shadcn_ui/lib/useToast'
import PartNode from '../../components/md_show/PartNode.vue'
import { partsOfMessages } from '../../components/md_show/goosePart'
import { visibleParts } from '../../components/md_show/toolPart'
const TERMINAL = ['completed', 'failed', 'cancelled']
const STATUS_TEXT = {
    queued: '正在准备回答',
    streaming: '正在生成',
    paused: '已暂停',
    waiting_approval: '等待审批',
    waiting_question: '等待回答',
    completed: '任务已完成',
    failed: '生成中断',
    cancelled: '已停止',
}
const props = defineProps({
    assistant: { type: Object, required: true },
    turnKey: { type: String, default: '' },
    isLastTurn: { type: Boolean, default: false },
})
const emit = defineEmits(['decision', 'answer', 'retry', 'delete', 'open-file', 'continue'])
const onDecision = (part, value) => emit('decision', part, value)
const onAnswer = (part, list) => emit('answer', part, list)
const status = computed(() => String(props.assistant.status || ''))
const active = computed(() => ['queued', 'streaming', 'paused', 'waiting_approval', 'waiting_question'].includes(status.value))
const streaming = computed(() => ['queued', 'streaming', 'paused'].includes(status.value))
const finished = computed(() => TERMINAL.includes(status.value))
const canContinue = computed(() => props.isLastTurn && (status.value === 'failed' || status.value === 'cancelled'))
const statusLabel = computed(() => STATUS_TEXT[status.value] || '任务已完成')
const parts = computed(() => partsOfMessages(props.assistant.messages, {
    sessionId: props.assistant.sessionId || '',
    decisions: props.assistant.decisions || {},
    expired: props.assistant.expired || {},
    streaming: streaming.value,
}))
const failure = computed(() => props.assistant.failure || null)
const usageText = computed(() => {
    const usage = Number(props.assistant.usage || 0)
    return usage > 0 ? usage.toLocaleString() : ''
})
const renderItems = computed(() => visibleParts(parts.value))
const copy = async () => {
    const text = parts.value.filter((part) => part.type === 'markdown' || part.type === 'text').map((part) => part.content || '').join('\n')
    if (!text) {
        toast.warning('暂无可复制的文本')
        return
    }
    try {
        await navigator.clipboard.writeText(text)
        toast.success('已复制')
    } catch {
        toast.error('复制失败')
    }
}
</script>

<style scoped>
.ai-message { display: flex; flex-direction: column; align-items: flex-start; gap: 8px; max-width: 100%; }
.ai-head { display: flex; align-items: center; gap: 8px; }
.agent-avatar { display: inline-flex; flex: none; align-items: center; justify-content: center; width: 22px; height: 22px; border-radius: 6px; overflow: hidden; }
.agent-avatar-img { width: 100%; height: 100%; object-fit: cover; border-radius: 6px; }
.agent-name { font-size: 13px; font-weight: 600; color: var(--foreground); }
.agent-detail { display: inline-flex; align-items: center; gap: 4px; font-size: 12px; color: var(--muted-foreground); }
.streaming-dot { width: 5px; height: 5px; border-radius: 50%; background: var(--primary); animation: streaming-pulse 1.3s ease-in-out infinite; }
.animated-ellipsis { display: inline-block; width: 1.6em; }
.animated-ellipsis::after { content: '...'; display: inline-block; width: 0; overflow: hidden; white-space: pre; vertical-align: bottom; animation: ellipsis-steps 1.2s steps(4, end) infinite; }
@keyframes streaming-pulse { 0%, 100% { opacity: 0.4; } 50% { opacity: 1; } }
@keyframes ellipsis-steps { to { width: 1.6em; } }

.thinking-hint { font-size: 12px; color: var(--muted-foreground); }
.failure-card { display: flex; align-items: flex-start; gap: 6px; width: 100%; padding: 8px 10px; border: 1px solid color-mix(in srgb, var(--destructive) 35%, transparent); border-radius: 6px; background: color-mix(in srgb, var(--destructive) 12%, transparent); color: var(--muted-foreground); font-size: 12px; line-height: 1.6; }
.failure-card svg { flex: none; margin-top: 1px; color: var(--destructive); }
.ai-actions { display: flex; align-items: center; justify-content: space-between; width: 100%; min-height: 26px; }
.action-left { display: flex; align-items: center; gap: 2px; opacity: 0.65; transition: opacity 0.15s ease; }
.ai-message:hover .action-left, .ai-actions:focus-within .action-left { opacity: 1; }
.action-left .mini-btn { width: 24px; height: 24px; }
.footer-details { margin-left: auto; font-size: 11px; color: var(--muted-foreground); opacity: 0; transition: opacity 0.15s ease; }
.ai-message:hover .footer-details, .ai-actions:focus-within .footer-details { opacity: 0.7; }
</style>