<template>
    <div v-if="settled" class="settled" role="button" tabindex="0" :aria-expanded="expanded" @click="expanded = !expanded" @keydown.enter.prevent="expanded = !expanded">
        <CircleCheck :size="12" />
        <span class="settled-text">命令审批 · {{ decidedText }}</span>
        <ChevronRight class="arrow" :class="{ open: expanded }" :size="12" />
    </div>
    <div v-if="!settled || expanded" class="confirm-bar" role="group" aria-label="命令审批">
        <TriangleAlert :size="14" class="confirm-icon" />
        <div class="confirm-main">
            <div class="confirm-title">需要你的批准<span class="confirm-tool">{{ part.tool || 'command_run' }}</span></div>
            <div v-if="command" class="confirm-command"><span class="prompt">$</span>{{ command }}</div>
            <div v-if="part.reason" class="confirm-reason">{{ part.reason }}</div>
            <div v-if="part.isHighRisky" class="confirm-risk">该操作可能影响本地文件或环境，请仔细核验。</div>
        </div>
        <div class="confirm-actions">
            <span v-if="part.expired" class="confirm-expired">审批已失效（超时或后端已重启），请重新发送本轮需求</span>
            <template v-else>
                <Button v-for="action in ACTIONS" :key="action.value" size="sm" :variant="action.type === 'primary' ? 'default' : 'outline'" :disabled="Boolean(part.decision) || part.status === 'pending'" :aria-label="action.label" @click="$emit('decision', part, action.value)">{{ action.label }}</Button>
                <span v-if="part.decision" class="confirm-done">{{ decidedText }}</span>
            </template>
        </div>
    </div>
</template>

<script setup>
import { computed, ref } from 'vue'
import { ChevronRight, CircleCheck, TriangleAlert } from 'lucide-vue-next'
import { Button } from '@/components/shadcn_ui/ui/button'
const ACTIONS = [{ value: 'once', label: '允许一次', type: 'primary' }, { value: 'always', label: '永久允许', type: 'default' }, { value: 'reject', label: '拒绝', type: 'default' }]
const DECIDED_TEXT = { pending: '处理中…', once: '已允许一次', always: '已永久允许（已写入白名单）', reject: '已拒绝', answer: '已回答' }
const props = defineProps({ part: { type: Object, required: true } })
defineEmits(['decision'])
const expanded = ref(false)
const settled = computed(() => Boolean(props.part.decision && props.part.decision !== 'pending') || props.part.status === 'allowed_once' || props.part.status === 'session_allowed' || props.part.status === 'denied')
const decidedText = computed(() => DECIDED_TEXT[String(props.part.decision || '')] || '已处理')
const command = computed(() => String(props.part.command || ''))
</script>

<style scoped>
.settled { display: flex; align-items: center; gap: 6px; padding: 4px 8px; border: 1px dashed var(--border); border-radius: 6px; color: var(--muted-foreground); font-size: 12px; cursor: pointer; user-select: none; }
.settled:focus-visible { outline: 1px solid var(--primary); outline-offset: -1px; }
.settled-text { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.settled .arrow { transition: transform 0.15s ease; }
.settled .arrow.open { transform: rotate(90deg); }
.confirm-bar { width: 100%; display: flex; align-items: flex-start; gap: 6px; padding: 6px 10px; border: 1px solid color-mix(in srgb, #f59e0b 35%, transparent); border-radius: 6px; background: color-mix(in srgb, #f59e0b 12%, transparent); font-size: 12px; }
.confirm-icon { flex: none; margin-top: 2px; color: #f59e0b; }
.confirm-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; }
.confirm-title { font-weight: 600; color: var(--foreground); }
.confirm-tool { margin-left: 6px; color: var(--muted-foreground); font-family: Consolas, Menlo, 'Courier New', monospace; font-weight: 400; }
.confirm-command { padding: 4px 8px; border-radius: 4px; background: var(--card); color: var(--foreground); font-family: Consolas, Menlo, 'Courier New', monospace; word-break: break-all; }
.prompt { margin-right: 6px; color: #22c55e; font-weight: 600; }
.confirm-reason { color: var(--muted-foreground); line-height: 1.6; }
.confirm-risk { color: #f59e0b; font-size: 11px; }
.confirm-actions { flex: none; display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
.confirm-done { font-size: 11px; color: var(--muted-foreground); }
.confirm-expired { font-size: 11px; color: #f59e0b; }
</style>