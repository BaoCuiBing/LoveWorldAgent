<template>
    <div class="unsupported-card">
        <CircleQuestionMark class="unsupported-icon" :size="14" />
        <div class="unsupported-main">
            <div class="unsupported-title">{{ title }}</div>
            <div class="unsupported-text">{{ message }}</div>
            <div v-if="diagnostic || showContinue" class="unsupported-actions">
                <button v-if="diagnostic" class="unsupported-more" type="button" :aria-expanded="open" @click="open = !open">{{ open ? '收起原始数据' : '查看原始数据' }}</button>
                <button v-if="showContinue" class="unsupported-continue" type="button" @click="onContinue">继续</button>
            </div>
            <pre v-if="open && diagnostic" class="unsupported-pre">{{ diagnostic }}</pre>
        </div>
    </div>
</template>

<script setup>
import { computed, ref } from 'vue'
import { CircleQuestionMark } from 'lucide-vue-next'
const props = defineProps({ part: { type: Object, required: true }, canContinue: { type: Boolean, default: false } })
const emit = defineEmits(['continue'])
const open = ref(false)
const continued = ref(false)
const showContinue = computed(() => props.canContinue && !continued.value && props.part.originalType === 'error')
const onContinue = () => {
    continued.value = true
    emit('continue')
}
const title = computed(() => props.part.title || '暂不支持该内容类型')
const message = computed(() => props.part.message || `原始类型：${props.part.originalType || '未知'}；后续有效内容仍会继续显示。`)
const diagnostic = computed(() => {
    if (!props.part.original) return ''
    try {
        return JSON.stringify(props.part.original, null, 2)
    } catch {
        return String(props.part.original)
    }
})
</script>

<style scoped>
.unsupported-card { display: flex; align-items: flex-start; gap: 6px; width: 100%; padding: 8px 10px; border: 1px dashed var(--border); border-radius: 6px; background: var(--muted); font-size: 12px; }
.unsupported-icon { flex: none; margin-top: 1px; color: var(--muted-foreground); }
.unsupported-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
.unsupported-title { color: var(--foreground); font-weight: 600; }
.unsupported-text { color: var(--muted-foreground); line-height: 1.6; word-break: break-word; }
.unsupported-actions { display: flex; align-items: center; gap: 12px; }
.unsupported-more { align-self: flex-start; padding: 0; border: none; background: transparent; color: var(--primary); font-size: 11px; cursor: pointer; }
.unsupported-continue { margin-left: auto; padding: 1px 8px; border: 1px solid color-mix(in srgb, #e6a23c 55%, transparent); border-radius: 4px; background: color-mix(in srgb, #e6a23c 18%, transparent); color: #e6a23c; font-size: 11px; font-weight: 500; cursor: pointer; }
.unsupported-continue:hover { background: color-mix(in srgb, #e6a23c 28%, transparent); }
.unsupported-pre { margin: 4px 0 0; padding: 6px 8px; max-height: 200px; overflow: auto; border-radius: 4px; background: var(--card); color: var(--muted-foreground); font-family: Consolas, Menlo, 'Courier New', monospace; font-size: 11px; white-space: pre-wrap; word-break: break-all; }
</style>