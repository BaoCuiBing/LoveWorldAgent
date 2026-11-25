<template>
    <div class="diff-card">
        <div class="diff-head" role="button" tabindex="0" :aria-expanded="open" @click="open = !open" @keydown.enter.prevent="open = !open">
            <FileText class="diff-icon" :size="14" />
            <span class="diff-path" :title="filePath">{{ fileName || filePath }}</span>
            <span class="diff-stat"><span class="add">+{{ additions }}</span><span class="del">-{{ deletions }}</span></span>
            <span v-if="part.truncated" class="diff-truncated">已截断</span>
            <Button class="diff-open" variant="ghost" size="icon-sm" aria-label="在文件面板打开" @click.stop="openFile">
                <FolderOpen :size="14" />
            </Button>
            <ChevronRight class="arrow" :class="{ open }" :size="12" />
        </div>
        <div v-if="open" class="diff-body">
            <div v-if="!hunks.length" class="diff-empty">（无逐行明细，仅统计增删行数）</div>
            <div v-for="(line, index) in visibleHunks" :key="index" class="diff-line" :class="`is-${line.type}`">
                <span class="line-no old">{{ line.oldLineNo || '' }}</span>
                <span class="line-no new">{{ line.newLineNo || '' }}</span>
                <span class="line-prefix">{{ prefixOf(line.type) }}</span>
                <span class="line-code">{{ line.content }}</span>
            </div>
            <button v-if="hunks.length > PREVIEW_LIMIT" class="diff-more" type="button" @click.stop="showAll = !showAll">{{ showAll ? '收起明细' : `展开全部 ${hunks.length} 行` }}</button>
        </div>
    </div>
</template>

<script setup>
import { computed, ref } from 'vue'
import { ChevronRight, FileText, FolderOpen } from 'lucide-vue-next'
import { Button } from '@/components/shadcn_ui/ui/button'
const PREVIEW_LIMIT = 60
const props = defineProps({ part: { type: Object, required: true } })
const emit = defineEmits(['open-file'])
const open = ref(false)
const showAll = ref(false)
const filePath = computed(() => String(props.part.filePath || props.part.meta?.path || ''))
const fileName = computed(() => filePath.value.replace(/\\/g, '/').split('/').pop() || '')
const additions = computed(() => Number(props.part.additions || 0))
const deletions = computed(() => Number(props.part.deletions || 0))
const hunks = computed(() => (Array.isArray(props.part.hunks) ? props.part.hunks : []))
const visibleHunks = computed(() => (showAll.value ? hunks.value : hunks.value.slice(0, PREVIEW_LIMIT)))
const prefixOf = (type) => (type === 'add' ? '+' : type === 'delete' ? '-' : ' ')
const openFile = () => { if (filePath.value) emit('open-file', { path: filePath.value }) }
</script>

<style scoped>
.diff-card { width: 100%; max-width: 100%; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; font-size: 12px; background: var(--card); }
.diff-head { display: flex; align-items: center; gap: 6px; padding: 6px 10px; background: var(--muted); cursor: pointer; user-select: none; }
.diff-head:focus-visible { outline: 1px solid var(--primary); outline-offset: -1px; }
.diff-icon { flex: none; color: var(--muted-foreground); }
.diff-path { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--foreground); font-weight: 600; font-family: Consolas, Menlo, 'Courier New', monospace; }
.diff-stat { flex: none; display: inline-flex; gap: 6px; font-family: Consolas, Menlo, 'Courier New', monospace; }
.diff-stat .add { color: #22c55e; }
.diff-stat .del { color: var(--destructive); }
.diff-truncated { flex: none; color: #f59e0b; font-size: 11px; }
.diff-open { flex: none; width: 22px; height: 22px; opacity: 0.65; transition: opacity 0.15s ease; }
.diff-open:hover { opacity: 1; }
.arrow { flex: none; color: var(--muted-foreground); transition: transform 0.15s ease; }
.arrow.open { transform: rotate(90deg); }
.diff-body { border-top: 1px solid var(--border); max-height: 340px; overflow: auto; font-family: Consolas, Menlo, 'Courier New', monospace; font-size: 11px; line-height: 1.6; }
.diff-empty { padding: 8px 10px; color: var(--muted-foreground); font-family: inherit; }
.diff-line { display: flex; align-items: flex-start; white-space: pre; }
.diff-line.is-add { background: color-mix(in srgb, #22c55e 12%, transparent); }
.diff-line.is-delete { background: color-mix(in srgb, var(--destructive) 12%, transparent); }
.line-no { flex: none; width: 34px; text-align: right; padding-right: 6px; color: var(--muted-foreground); user-select: none; }
.line-prefix { flex: none; width: 16px; text-align: center; user-select: none; font-weight: 700; }
.is-add .line-prefix { color: #22c55e; }
.is-delete .line-prefix { color: var(--destructive); }
.line-code { flex: 1; padding-right: 10px; color: var(--foreground); }
.is-delete .line-code { color: var(--muted-foreground); text-decoration: line-through; }
.diff-more { width: 100%; padding: 4px; border: none; border-top: 1px dashed var(--border); background: transparent; color: var(--primary); font-size: 11px; cursor: pointer; font-family: inherit; }
</style>