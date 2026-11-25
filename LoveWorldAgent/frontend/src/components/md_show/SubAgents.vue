<template>
    <div class="subagent-card">
        <div class="subagent-head" role="button" tabindex="0" :aria-expanded="open" @click="open = !open" @keydown.enter.prevent="open = !open" @keydown.space.prevent="open = !open">
            <LoaderCircle v-if="running" class="subagent-icon spinning" :size="14" />
            <CircleX v-else-if="failed" class="subagent-icon failed" :size="14" />
            <CircleCheck v-else class="subagent-icon done" :size="14" />
            <span class="subagent-title" :class="{ failed }">{{ name }}</span>
            <span class="subagent-status" :class="{ failed }">{{ statusText }}</span>
            <span class="subagent-count">{{ toolCount }} 步</span>
            <ChevronRight class="arrow" :class="{ open }" :size="12" />
        </div>
        <div v-if="open" class="subagent-body">
            <div v-if="!blocks.length" class="subagent-empty">暂无步骤</div>
            <PartNode v-for="item in blocks" :key="item.partKey" :part="item" @decision="(p, value) => $emit('decision', p, value)" @answer="(p, list) => $emit('answer', p, list)" @open-file="(payload) => $emit('open-file', payload)" />
        </div>
    </div>
</template>

<script setup>
import { computed, ref, watch } from 'vue'
import { ChevronRight, CircleCheck, CircleX, LoaderCircle } from 'lucide-vue-next'
import PartNode from './PartNode.vue'
import { partsOfMessages } from './goosePart'
import { visibleParts } from './toolPart'
const props = defineProps({ part: { type: Object, required: true } })
defineEmits(['decision', 'answer', 'open-file'])
const open = ref(Boolean(props.part.running))
const messages = computed(() => (Array.isArray(props.part.messages) ? props.part.messages : []))
const running = computed(() => Boolean(props.part.running))
const failed = computed(() => !running.value && Boolean(props.part.failed))
const name = computed(() => String(props.part.agentName || '').trim() || '子智能体')
const statusText = computed(() => (running.value ? '运行中' : failed.value ? '失败' : '已完成'))
const toolCount = computed(() => Number(props.part.toolCount) || 0)
/** 卡内每条子智能体消息的 parts 全部正常渲染（内置工具会渲染成 BuildToolCall 卡，允许嵌套） */
const blocks = computed(() => {
    const list = messages.value
    if (!list.length) return []
    return visibleParts(partsOfMessages(list, { ignoreSubagent: true, streaming: running.value }))
})
// 运行中自动展开，结束默认折叠
watch(running, (value) => { if (messages.value.length) open.value = Boolean(value) })
</script>

<style scoped>
.subagent-card { width: 100%; max-width: 100%; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; font-size: 12px; background: var(--card); }
.subagent-head { display: flex; align-items: center; gap: 6px; padding: 6px 10px; background: var(--muted); cursor: pointer; user-select: none; }
.subagent-head:focus-visible { outline: 1px solid var(--primary); outline-offset: -1px; }
.subagent-icon { flex: none; color: var(--muted-foreground); }
.subagent-icon.failed { color: var(--destructive); }
.subagent-icon.done { color: #22c55e; }
.subagent-icon.spinning { color: var(--primary); animation: subagent-spin 1s linear infinite; }
.subagent-title { flex: none; color: var(--foreground); font-weight: 600; }
.subagent-title.failed { color: var(--destructive); }
.subagent-status { flex: none; color: var(--muted-foreground); }
.subagent-status.failed { color: var(--destructive); }
.subagent-count { flex: none; color: var(--muted-foreground); }
.arrow { margin-left: auto; flex: none; color: var(--muted-foreground); transition: transform 0.15s ease; }
.arrow.open { transform: rotate(90deg); }
.subagent-body { display: flex; flex-direction: column; gap: 8px; max-height: 320px; overflow-y: auto; padding: 8px 10px; border-top: 1px solid var(--border); }
.subagent-empty { padding: 8px 0; color: var(--muted-foreground); text-align: center; }
@keyframes subagent-spin { to { transform: rotate(360deg); } }
</style>
