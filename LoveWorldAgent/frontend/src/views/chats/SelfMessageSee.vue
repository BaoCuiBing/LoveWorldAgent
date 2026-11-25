<template>
    <div class="self-message">
        <div v-if="attachments.length" class="self-attachments">
            <div v-for="item in attachments" :key="item.name" class="self-attach" :class="{ 'is-image': item.isImage }" :title="item.name">
                <img v-if="item.isImage && item.url" class="self-attach-thumb" :src="item.url" :alt="item.name" />
                <span v-else class="self-attach-name">{{ item.name }}</span>
            </div>
        </div>
        <div class="self-bubble">
            <template v-for="(segment, index) in segments" :key="index">
                <span v-if="segment.type === 'text'" class="self-segment-text">{{ segment.value }}</span>
                <button v-else-if="segment.type === 'ref'" class="file-ref-card" type="button" :title="segment.path" @click="onOpenFile({ path: segment.path, type: segment.kind })">
                    <img class="file-ref-icon" :src="fileIconOf(segment.name, segment.kind)" :alt="segment.name" />
                    <span class="file-ref-name">{{ segment.name }}</span>
                    <span class="file-ref-path">{{ segment.path }}</span>
                    <ChevronRight class="file-ref-arrow" :size="12" />
                </button>
                <span v-else class="self-segment-attach">📎 {{ segment.name }}</span>
            </template>
        </div>
        <div class="self-meta">
            <span class="self-time">{{ message.time }}</span>
            <span class="self-actions">
                <Button class="mini-btn" variant="ghost" size="icon-sm" aria-label="复制" @click="copy">
                    <Copy :size="14" />
                </Button>
                <Button class="mini-btn" variant="ghost" size="icon-sm" aria-label="撤回" @click="$emit('recall', message)">
                    <CornerUpLeft :size="14" />
                </Button>
            </span>
        </div>
    </div>
</template>

<script setup>
import { computed } from 'vue'
import { ChevronRight, Copy, CornerUpLeft } from 'lucide-vue-next'
import { Button } from '@/components/shadcn_ui/ui/button'
import { toast } from '@/components/shadcn_ui/lib/useToast'
import { uploadPreviewUrl } from '../../api/request_data'
import { fileIconOf } from '../../assets/fileIcons'
// 用户消息片段解析：把 #File: / #Folder: / 📎 标记切成文本、引用、附件三类片段。
// 模块级正则只编译一次，组件内用 computed 缓存结果，避免每次渲染重复扫描。
const TOKEN_RE = /#(File|Folder):([^\s#]+)|📎\s*(\S+)/g
const baseName = (value) => String(value || '').replace(/[\\/]+$/, '').split(/[\\/]/).pop() || String(value || '')
const parseContentSegments = (text) => {
    const source = String(text || '')
    const segments = []
    let cursor = 0
    const regex = new RegExp(TOKEN_RE.source, 'g')
    let match
    while ((match = regex.exec(source)) !== null) {
        if (match.index > cursor) segments.push({ type: 'text', value: source.slice(cursor, match.index) })
        if (match[1]) {
            const value = match[2]
            segments.push({ type: 'ref', kind: match[1] === 'Folder' ? 'folder' : 'file', path: value, name: baseName(value) })
        } else {
            segments.push({ type: 'attachment', name: match[3] || '附件' })
        }
        cursor = regex.lastIndex
    }
    if (cursor < source.length) segments.push({ type: 'text', value: source.slice(cursor) })
    return segments.length ? segments : [{ type: 'text', value: source }]
}
const props = defineProps({ message: { type: Object, required: true } })
const emit = defineEmits(['recall', 'open-file'])
const attachments = computed(() => (Array.isArray(props.message.attachments) ? props.message.attachments : []).map((item) => ({ ...item, url: item.url || (item.isImage ? uploadPreviewUrl(item.name) : '') })))
const segments = computed(() => parseContentSegments(props.message.content))
const onOpenFile = (payload) => emit('open-file', payload)
const copy = async () => {
    try {
        await navigator.clipboard.writeText(props.message.content || '')
        toast.success('已复制')
    } catch {
        toast.error('复制失败')
    }
}
</script>

<style scoped>
.self-message { display: flex; flex-direction: column; align-items: flex-end; gap: 4px; }
.self-attachments { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 6px; max-width: 72%; }
.self-attach { display: flex; align-items: center; height: 24px; max-width: 160px; padding: 0 8px; border: 1px solid var(--border); border-radius: 6px; background: var(--muted); }
.self-attach.is-image { width: 48px; height: 48px; padding: 0; }
.self-attach-thumb { width: 100%; height: 100%; border-radius: 6px; object-fit: cover; }
.self-attach-name { overflow: hidden; font-size: 11px; text-overflow: ellipsis; white-space: nowrap; color: var(--muted-foreground); }
.self-bubble { max-width: 72%; padding: 10px 14px; border-radius: 10px; background: var(--muted); color: var(--foreground); font-size: 13px; line-height: 1.7; white-space: pre-wrap; word-break: break-word; }
.self-segment-text { white-space: pre-wrap; word-break: break-word; }
.file-ref-card { display: inline-flex; align-items: center; gap: 6px; max-width: 100%; min-height: 24px; margin: 0 2px; padding: 0 8px; border: 1px solid var(--border); border-radius: 6px; background: var(--muted); cursor: pointer; text-align: left; vertical-align: middle; }
.file-ref-card:hover { background: var(--accent); border-color: var(--border); }
.file-ref-icon { flex: none; display: block; width: 14px; height: 14px; }
.file-ref-name { flex: none; max-width: 180px; overflow: hidden; font-size: 12px; text-overflow: ellipsis; white-space: nowrap; color: var(--foreground); }
.file-ref-path { flex: 1; min-width: 0; overflow: hidden; font-size: 11px; text-overflow: ellipsis; white-space: nowrap; color: var(--muted-foreground); }
.file-ref-arrow { flex: none; opacity: 0; color: var(--muted-foreground); transition: opacity 0.15s ease; }
.file-ref-card:hover .file-ref-arrow { opacity: 1; }
.self-segment-attach { margin: 0 2px; font-size: 12px; color: var(--muted-foreground); }
.self-meta { display: flex; align-items: center; gap: 8px; min-height: 24px; padding-right: 2px; color: var(--muted-foreground); font-size: 11px; }
.self-time { opacity: 0; transition: opacity 0.15s ease; }
.self-message:hover .self-time { opacity: 1; }
.self-actions { display: inline-flex; opacity: 0; transition: opacity 0.15s ease; }
.self-message:hover .self-actions { opacity: 1; }
.self-actions .mini-btn { width: 24px; height: 24px; }
</style>
