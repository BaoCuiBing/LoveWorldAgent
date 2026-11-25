<template>
    <div ref="listRef" class="message-list" @scroll="onScroll">
        <button v-if="hiddenCount > 0" class="load-earlier" type="button" @click="loadEarlier">
            加载更早的 {{ Math.min(EARLIER_STEP, hiddenCount) }} 轮（共 {{ hiddenCount }} 轮未显示）
        </button>
        <TurnItem v-for="turn in visibleTurns" :key="turn.turnKey" :turn="turn" :rollback="turn.turnKey === rollbackId" :is-last="turn.turnKey === lastTurnKey" @recall="(payload) => $emit('recall', payload)" @delete="(payload) => $emit('delete', payload)" @retry="(payload) => $emit('retry', payload)" @rollback-confirm="(payload) => $emit('rollback-confirm', payload)" @rollback-cancel="$emit('rollback-cancel')" @decision="(payload, part, decision) => $emit('decision', payload, part, decision)" @answer="(payload, part, answers) => $emit('answer', payload, part, answers)" @open-file="(payload) => $emit('open-file', payload)" @continue="(payload) => $emit('continue', payload)" />
        <div v-if="!turns.length" class="message-empty">
            <img class="message-empty-logo" :src="dragon" alt="Agent" />
            <div class="message-empty-title">Agent</div>
            <div class="message-empty-subtitle">轻松应对复杂项目开发</div>
            <ul class="message-empty-points">
                <li>读懂整个项目结构，按需求规划并逐步落地</li>
                <li>读写项目文件、执行终端命令，交付生产级代码</li>
                <li>内置浏览器与规则、技能，长任务持续推进</li>
            </ul>
        </div>
    </div>
</template>

<script setup>
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import dragon from '../../assets/dragon.png'
import TurnItem from './TurnItem.vue'
const props = defineProps({ turns: { type: Array, default: () => [] }, rollbackId: { type: String, default: '' } })
defineEmits(['recall', 'delete', 'retry', 'rollback-confirm', 'rollback-cancel', 'decision', 'answer', 'open-file', 'continue'])
const listRef = ref(null)
const STICK_THRESHOLD = 200  // 判定「用户在看底部」的容差：单帧内容增长往往远大于 80px，阈值过紧会让自动贴底一旦脱队就永不追平
const stick = ref(true)
const WINDOW_SIZE = 40  // 虚拟列表窗口：只渲染最近 40 轮，避免长会话全量 DOM 卡顿
const EARLIER_STEP = 40
const visibleCount = ref(WINDOW_SIZE)
const visibleTurns = computed(() => (props.turns.length > visibleCount.value ? props.turns.slice(-visibleCount.value) : props.turns))
const lastTurnKey = computed(() => (props.turns.length ? props.turns[props.turns.length - 1].turnKey : ''))
const hiddenCount = computed(() => Math.max(0, props.turns.length - visibleCount.value))
const loadEarlier = () => {
    const el = listRef.value
    const before = el ? { top: el.scrollTop, height: el.scrollHeight } : null
    visibleCount.value += EARLIER_STEP
    if (el && before) nextTick(() => { el.scrollTop = el.scrollHeight - before.height + before.top })
}
const atBottom = (el) => el.scrollHeight - el.scrollTop - el.clientHeight < STICK_THRESHOLD
const scrollToBottom = () => nextTick(() => {
    const el = listRef.value
    if (el) el.scrollTop = el.scrollHeight
})
const onScroll = () => {
    const el = listRef.value
    if (el) stick.value = atBottom(el)  // 用户手动上滚即停止自动跟随，滚回底部即恢复跟随
}
watch(() => props.turns, () => { if (stick.value) scrollToBottom() }, { deep: true })
watch(() => props.turns.length, (now, prev) => {
    if (now < prev) visibleCount.value = WINDOW_SIZE
})
onMounted(() => {
    stick.value = true
    scrollToBottom()
})
defineExpose({ scrollToBottom })  // 发送新消息由外层强制贴底（与 stick 无关），保证用户永远看到自己刚发的那一条
</script>

<style scoped>
.message-list { flex: 1; min-height: 0; padding: 8px 4px 20px; overflow-y: auto; overflow-x: hidden; display: flex; flex-direction: column; gap: 18px; scroll-padding-bottom: 20px; }
.load-earlier { align-self: center; padding: 5px 14px; border: 1px solid var(--border); border-radius: 14px; background: var(--muted); color: var(--muted-foreground); font-size: 12px; cursor: pointer; }
.load-earlier:hover { color: var(--primary); border-color: color-mix(in srgb, var(--primary) 20%, transparent); }
.message-empty { display: flex; flex-direction: column; align-items: center; gap: 8px; margin: auto; padding: 24px 16px; text-align: center; }
.message-empty-logo { width: 72px; height: 72px; object-fit: contain; }
.message-empty-title { font-size: 16px; font-weight: 600; color: var(--foreground); }
.message-empty-subtitle { font-size: 13px; color: var(--muted-foreground); }
.message-empty-points { display: flex; flex-direction: column; gap: 4px; margin: 8px 0 0; padding: 0; list-style: none; }
.message-empty-points li { position: relative; padding-left: 14px; font-size: 12px; line-height: 1.8; color: var(--muted-foreground); }
.message-empty-points li::before { content: ''; position: absolute; left: 2px; top: 9px; width: 4px; height: 4px; border-radius: 50%; background: var(--primary); }
</style>