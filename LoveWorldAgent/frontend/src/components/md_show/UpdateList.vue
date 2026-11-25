<template>
    <div v-if="items.length" class="update-list">
        <button class="ul-head" type="button" :aria-expanded="open" @click="open = !open">
            <ChevronRight class="ul-arrow" :class="{ open }" :size="12" />
            <Clock3 v-if="running" class="ul-icon running" :size="14" />
            <CircleCheck v-else class="ul-icon done" :size="14" />
            <span class="ul-title">{{ title }}</span>
            <span class="ul-count">({{ doneCount }}/{{ items.length }})</span>
            <span class="ul-progress"><span class="ul-progress-inner" :style="{ width: percent + '%' }" /></span>
            <span v-if="elapsedText" class="ul-elapsed">{{ elapsedText }}</span>
            <span class="ul-hint">{{ open ? '收起' : '展开' }}</span>
        </button>
        <div v-if="!open" class="ul-preview">当前进度：{{ doneCount }}/{{ items.length }} · 点击展开查看任务清单</div>
        <div v-show="open" class="ul-body">
            <div v-for="(item, index) in items" :key="item.id || index" class="ul-row" :class="{ focused: item.status === 'in_progress' }" :title="item.title || item.content">
                <CircleCheck v-if="item.status === 'completed'" class="ul-state done" :size="13" />
                <Clock3 v-else-if="item.status === 'in_progress'" class="ul-state running" :size="13" />
                <CircleCheck v-else class="ul-state pending" :size="13" />
                <span class="ul-text" :class="{ finished: item.status === 'completed' }">{{ item.title || item.content }}</span>
                <span v-if="item.status === 'in_progress'" class="ul-badge">进行中</span>
            </div>
            <div v-if="changedFiles.length" class="ul-files">
                <div class="ul-files-head">
                    <span>{{ changedFiles.length }} 个变更文件</span>
                    <span class="ul-diff-stat">+{{ totalAdditions }} -{{ totalDeletions }}</span>
                </div>
                <div v-for="(file, index) in changedFiles" :key="index" class="ul-file-row">
                    <span class="ul-file-name">{{ file.name }}</span>
                    <span class="ul-file-path">{{ file.path }}</span>
                    <span class="ul-file-add">+{{ file.additions || 0 }}</span>
                    <span class="ul-file-del">-{{ file.deletions || 0 }}</span>
                </div>
            </div>
        </div>
    </div>
</template>

<script setup>
import { computed, ref } from 'vue'
import { ChevronRight, CircleCheck, Clock3 } from 'lucide-vue-next'
const props = defineProps({ part: { type: Object, default: () => ({}) }, changedFiles: { type: Array, default: () => [] } })
const open = ref(false)  // 默认收起，避免长清单挤占对话流
const items = computed(() => (Array.isArray(props.part && props.part.todos) ? props.part.todos : Array.isArray(props.part && props.part.items) ? props.part.items : []))
const title = computed(() => String((props.part && props.part.title) || '任务待办清单'))
const elapsedText = computed(() => String((props.part && props.part.elapsedText) || ''))
const doneCount = computed(() => items.value.filter((item) => String(item.status || '').toLowerCase() === 'completed').length)
const running = computed(() => items.value.some((item) => String(item.status || '').toLowerCase() === 'in_progress'))
const percent = computed(() => (items.value.length ? Math.round((doneCount.value / items.value.length) * 100) : 0))
const totalAdditions = computed(() => props.changedFiles.reduce((sum, file) => sum + Number(file.additions || 0), 0))
const totalDeletions = computed(() => props.changedFiles.reduce((sum, file) => sum + Number(file.deletions || 0), 0))
</script>

<style scoped>
.update-list { width: 100%; border: 1px solid var(--border); border-radius: 6px; background: var(--card); font-size: 12px; }
.ul-head { display: flex; align-items: center; gap: 6px; width: 100%; padding: 6px 10px; border: none; border-radius: 6px; background: transparent; color: var(--muted-foreground); cursor: pointer; transition: background 0.15s ease; }
.ul-head:hover { background: var(--accent); }
.ul-arrow { flex: none; transition: transform 0.15s ease; }
.ul-arrow.open { transform: rotate(90deg); }
.ul-icon { flex: none; }
.ul-icon.running { color: #f59e0b; }
.ul-icon.done { color: #22c55e; }
.ul-title { flex: none; font-weight: 600; color: var(--foreground); }
.ul-count { flex: none; color: var(--muted-foreground); }
.ul-progress { flex: 1; min-width: 32px; max-width: 140px; height: 4px; margin-left: 4px; border-radius: 2px; background: var(--muted); overflow: hidden; }
.ul-progress-inner { display: block; height: 100%; border-radius: 2px; background: var(--primary); transition: width 0.15s ease; }
.ul-elapsed { flex: none; color: var(--muted-foreground); font-size: 11px; font-family: Consolas, Menlo, 'Courier New', monospace; }
.ul-hint { flex: none; margin-left: auto; color: var(--muted-foreground); font-size: 11px; }
.ul-preview { padding: 4px 10px 8px 30px; color: var(--muted-foreground); font-size: 11px; }
.ul-body { display: flex; flex-direction: column; gap: 2px; padding: 4px 10px 8px 24px; border-top: 1px solid var(--border); }
.ul-row { display: flex; align-items: center; gap: 6px; min-height: 20px; padding: 2px 4px; border-radius: 4px; }
.ul-row.focused { background: color-mix(in srgb, #f59e0b 12%, transparent); }
.ul-state { flex: none; }
.ul-state.done { color: #22c55e; }
.ul-state.running { color: #f59e0b; }
.ul-state.pending { color: var(--muted-foreground); }
.ul-text { min-width: 0; color: var(--foreground); line-height: 1.6; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ul-text.finished { color: var(--muted-foreground); opacity: 0.75; }
.ul-badge { flex: none; padding: 0 4px; border-radius: 3px; background: color-mix(in srgb, #f59e0b 12%, transparent); color: #f59e0b; font-size: 10px; }
.ul-files { display: flex; flex-direction: column; gap: 3px; margin-top: 6px; padding-top: 6px; border-top: 1px dashed var(--border); }
.ul-files-head { display: flex; align-items: center; justify-content: space-between; color: var(--muted-foreground); font-size: 11px; }
.ul-diff-stat { color: #22c55e; font-family: Consolas, Menlo, 'Courier New', monospace; }
.ul-file-row { display: flex; align-items: center; gap: 6px; min-width: 0; font-size: 11px; }
.ul-file-name { color: var(--foreground); }
.ul-file-path { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--muted-foreground); }
.ul-file-add { color: #22c55e; }
.ul-file-del { color: var(--destructive); }
</style>