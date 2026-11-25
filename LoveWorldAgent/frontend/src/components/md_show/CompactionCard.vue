<template>
    <div class="compact-card" :class="{ open }">
        <button class="compact-head" type="button" :aria-expanded="open" @click="open = !open">
            <ListFilter class="compact-icon" :size="14" />
            <span class="compact-label">压缩</span>
            <ChevronRight class="compact-arrow" :class="{ open }" :size="12" />
        </button>
        <div v-if="open" class="compact-body">
            <div class="compact-stats">
                <span>压缩前：{{ formatNum(part.tokens_before) }} Tokens</span>
                <span>压缩后：{{ formatNum(part.tokens_after) }} Tokens</span>
                <span>隐藏旧消息：{{ Number(part.hidden_count || 0) }} 条</span>
                <span>基线：{{ formatNum(part.baseline) }}</span>
            </div>
            <div v-if="summaryText" class="compact-section">
                <div class="compact-sec-title">结构化摘要</div>
                <div class="compact-summary"><MdShow :content="summaryText" /></div>
            </div>
            <div v-if="factsList.length" class="compact-section">
                <div class="compact-sec-title">关键事实（{{ factsList.length }} 条）</div>
                <ul class="compact-facts">
                    <li v-for="(fact, idx) in factsList" :key="idx">{{ fact }}</li>
                </ul>
            </div>
        </div>
    </div>
</template>

<script setup>
import { computed, ref } from 'vue'
import { ChevronRight, ListFilter } from 'lucide-vue-next'
import MdShow from './MdShow.vue'
const props = defineProps({ part: { type: Object, required: true } })
const open = ref(false)
const formatNum = (val) => Number(val || 0).toLocaleString('en-US')
const summaryText = computed(() => String((props.part && props.part.summary) || '').trim())
const factsList = computed(() => (Array.isArray(props.part && props.part.facts) ? props.part.facts.filter(Boolean) : []))
</script>

<style scoped>
.compact-card { width: 100%; border: 1px solid var(--border); border-radius: 8px; background: var(--card); overflow: hidden; }
.compact-head { display: inline-flex; align-items: center; gap: 6px; width: 100%; padding: 8px 12px; border: none; background: transparent; color: var(--foreground); font-size: 13px; text-align: left; cursor: pointer; }
.compact-head:hover { background: var(--accent); }
.compact-icon { flex: none; color: var(--muted-foreground); }
.compact-label { font-weight: 500; }
.compact-arrow { flex: none; color: var(--muted-foreground); transition: transform 0.15s ease; }
.compact-arrow.open { transform: rotate(90deg); }
.compact-body { display: flex; flex-direction: column; gap: 10px; padding: 10px 12px; border-top: 1px solid var(--border); background: var(--muted); font-size: 12px; }
.compact-stats { display: flex; flex-wrap: wrap; gap: 12px; color: var(--muted-foreground); font-variant-numeric: tabular-nums; }
.compact-sec-title { margin-bottom: 4px; font-weight: 600; color: var(--foreground); }
.compact-summary { max-height: 260px; overflow: auto; padding: 8px 10px; border-radius: 6px; background: var(--card); border: 1px solid var(--border); }
.compact-facts { margin: 0; padding-left: 18px; display: flex; flex-direction: column; gap: 3px; color: var(--foreground); line-height: 1.5; }
</style>
