<template>
    <div class="tool-card" :class="{ open }">
        <div class="tool-head" :class="{ plain: !canExpand }" :role="canExpand ? 'button' : undefined" :tabindex="canExpand ? 0 : -1" :aria-expanded="canExpand ? open : undefined" @click="canExpand && toggle()" @keydown.enter.prevent="canExpand && toggle()" @keydown.space.prevent="canExpand && toggle()">
            <Trash2 v-if="kind === 'delete'" class="tool-icon" :size="14" />
            <FolderTree v-else-if="kind === 'list'" class="tool-icon" :size="14" />
            <NotebookText v-else class="tool-icon" :size="14" />
            <span class="tool-title">{{ title }}</span>
            <span v-if="statusText" class="tool-status" :class="status">
                <Clock3 v-if="status === 'running'" :size="14" />
                <CircleX v-else-if="status === 'failed'" :size="14" />
                <TriangleAlert v-else-if="status === 'pending'" :size="14" />
                <CircleCheck v-else :size="14" />
                {{ statusText }}
            </span>
            <span v-if="metaText" class="tool-summary">{{ metaText }}</span>
            <span v-if="durationText" class="tool-duration">{{ durationText }}</span>
            <Button v-if="path" class="file-open" variant="ghost" size="icon-sm" aria-label="在文件面板打开" @click.stop="openFile">
                <FolderOpen :size="14" />
            </Button>
            <ChevronRight v-if="canExpand" class="arrow" :class="{ open }" :size="12" />
        </div>
        <div v-if="canExpand && open" class="tool-body">
            <div v-if="kind === 'list'" class="file-list">
                <div v-for="item in entries" :key="item.name" class="file-row">
                    <Folder v-if="item.type === 'dir'" class="file-row-icon" :size="12" />
                    <FileText v-else class="file-row-icon" :size="12" />
                    <span class="file-name" :title="item.name">{{ item.name }}</span>
                    <span v-if="item.type === 'file'" class="file-size">{{ sizeText(item.size) }}</span>
                </div>
                <div v-if="!entries.length" class="file-empty">{{ outputText || '（空目录）' }}</div>
            </div>
            <div v-else class="file-result" :class="{ bad: status === 'failed' }">{{ outputText || '（无输出）' }}</div>
        </div>
    </div>
</template>

<script setup>
import { computed, ref } from 'vue'
import { ChevronRight, CircleCheck, CircleX, Clock3, FileText, Folder, FolderOpen, FolderTree, NotebookText, Trash2, TriangleAlert } from 'lucide-vue-next'
import { Button } from '@/components/shadcn_ui/ui/button'
import { STATUS_LABEL, argsOf, firstLine, formatDuration, outputOf, parseArgs, resolveStatus, resultOutput, toolNameOf } from './toolPart'
const props = defineProps({ part: { type: Object, required: true } })
const emit = defineEmits(['open-file'])
const ACTION_TEXT = { file_list: '文件列表', file_read: '文件读取', file_delete: '文件删除' }
const open = ref(false)
const toggle = () => { open.value = !open.value }
const status = computed(() => resolveStatus(props.part))
const statusText = computed(() => STATUS_LABEL[status.value] || '')
const durationText = computed(() => formatDuration(props.part.durationMs))
const toolName = computed(() => toolNameOf(props.part))
const meta = computed(() => props.part.meta || {})
const kind = computed(() => (toolName.value === 'file_delete' ? 'delete' : toolName.value === 'file_list' ? 'list' : toolName.value === 'file_read' ? 'read' : 'other'))
const canExpand = computed(() => kind.value === 'list' || kind.value === 'other')
const path = computed(() => String(meta.value.path || (parseArgs(argsOf(props.part)) || {}).path || ''))
const lineRange = computed(() => (meta.value.startLine && meta.value.endLine ? `L${meta.value.startLine}-${meta.value.endLine}` : ''))
const bytes = computed(() => Number(meta.value.bytes || 0) || '')
const title = computed(() => `${ACTION_TEXT[toolName.value] || '文件操作'} · ${path.value || toolName.value}`)
const outputText = computed(() => resultOutput(outputOf(props.part)))
const entries = computed(() => (Array.isArray(meta.value.items) ? meta.value.items : []))
const metaText = computed(() => {
    if (status.value === 'failed') return firstLine(outputText.value)
    if (kind.value === 'delete') return '已删除'
    if (kind.value === 'read') return lineRange.value
    if (kind.value === 'list') return entries.value.length ? `${entries.value.length} 项` : ''
    return bytes.value ? `${bytes.value} 字节` : ''
})
const sizeText = (value) => {
    const size = Number(value || 0)
    if (!size) return ''
    if (size < 1024) return `${size} B`
    if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`
    return `${(size / 1024 / 1024).toFixed(1)} MB`
}
const openFile = () => { if (path.value) emit('open-file', { path: path.value }) }
</script>

<style scoped>
.tool-card { width: 100%; max-width: 100%; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; font-size: 12px; background: var(--card); }
.tool-head { display: flex; align-items: center; gap: 6px; padding: 6px 10px; background: var(--muted); cursor: pointer; user-select: none; }
.tool-head.plain { cursor: default; }
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
.file-open { flex: none; width: 22px; height: 22px; opacity: 0.65; transition: opacity 0.15s ease; }
.file-open:hover { opacity: 1; }
.file-list { display: flex; flex-direction: column; gap: 2px; }
.file-row { display: flex; align-items: center; gap: 6px; min-height: 20px; padding: 2px 4px; border-radius: 4px; }
.file-row:hover { background: var(--accent); }
.file-row-icon { flex: none; color: var(--muted-foreground); }
.file-name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--foreground); }
.file-size { flex: none; color: var(--muted-foreground); font-size: 11px; }
.file-empty { color: var(--muted-foreground); }
.file-result { color: var(--foreground); line-height: 1.6; white-space: pre-wrap; word-break: break-word; }
.file-result.bad { color: var(--destructive); }
</style>