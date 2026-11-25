<template>
    <div class="file-viewer" @keydown.capture="onKeydown">
        <div v-if="!activeTab" class="viewer-empty">选择右侧文件查看内容</div>
        <template v-else>
            <div class="viewer-tabs">
                <div v-for="tab in tabs" :key="tab.rel" class="viewer-tab" :class="{ active: tab.rel === activeRel, dirty: tabDirty(tab) }" :title="tabDirty(tab) ? `${tab.rel}（有未保存的修改，Ctrl+S 保存）` : tab.rel" @click="emit('activate', tab.rel)">
                    <img class="tab-icon" :src="fileIconOf(tab.rel)" :alt="languageOf(tab.rel)" />
                    <span class="tab-name">{{ tab.name }}</span>
                    <span v-if="tabDirty(tab)" class="tab-dot" aria-hidden="true" />
                    <span class="tab-close" title="关闭" @click.stop="emit('close', tab.rel)">×</span>
                </div>
            </div>
            <div v-if="activeTab.loading" class="viewer-empty">加载中…</div>
            <div v-else class="viewer-body">
                <OfficeView v-if="binaryActive" :rel="activeRel" :version="versions[activeRel] || 0" />
                <CodeEditor v-else ref="editorRef" v-model="text" :language="language" :read-only="false" height="100%" />
            </div>
        </template>
    </div>
</template>

<script setup>
import { computed, nextTick, ref, watch } from 'vue'
import CodeEditor from '../editor/CodeEditor.vue'
import OfficeView from '../editor/OfficeView.vue'
import { fileIconOf } from '../../assets/fileIcons'
import { langForPath } from '../../utils/codeLang.js'
const props = defineProps({
    tabs: { type: Array, default: () => [] },
    activeRel: { type: String, default: '' },
    versions: { type: Object, default: () => ({}) },
})
const emit = defineEmits(['activate', 'close', 'save'])
const BINARY_EXTS = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg', 'ico', 'pdf', 'doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx']
const extOf = (rel) => {
    const base = String(rel || '').split(/[\\/]/).pop() || ''
    const idx = base.lastIndexOf('.')
    return idx > 0 ? base.slice(idx + 1).toLowerCase() : ''
}
const isBinary = (rel) => BINARY_EXTS.includes(extOf(rel))
const activeTab = computed(() => props.tabs.find((tab) => tab.rel === props.activeRel) || null)
const languageOf = (rel) => langForPath(rel)
const language = computed(() => languageOf(props.activeRel))
const binaryActive = computed(() => Boolean(activeTab.value && (activeTab.value.binary || isBinary(activeTab.value.rel))))
const tabDirty = (tab) => Boolean(tab) && !tab.binary && !isBinary(tab.rel) && tab.content !== tab.saved
const dirty = computed(() => tabDirty(activeTab.value))
const text = computed({
    get: () => (activeTab.value ? activeTab.value.content : ''),
    set: (value) => { if (activeTab.value) activeTab.value.content = value },
})
const isDirty = () => dirty.value
const editorRef = ref(null)
watch(() => props.activeRel, async (rel) => {  // 打开/切换文件后激活编辑器（含右栏自动打开场景）
    if (!rel) return
    await nextTick()
    await nextTick()
    if (editorRef.value && !binaryActive.value) editorRef.value.focus()
})
const save = () => {
    if (!activeTab.value || binaryActive.value || !dirty.value) return
    emit('save', { rel: activeTab.value.rel, text: activeTab.value.content })
}
const onKeydown = (event) => {
    if (binaryActive.value) return
    if (!(event.ctrlKey || event.metaKey)) return
    if (event.key !== 's' && event.key !== 'S') return
    event.preventDefault()
    event.stopPropagation()
    save()
}
defineExpose({ isDirty, save })
</script>

<style scoped>
.file-viewer { display: flex; flex-direction: column; min-height: 0; height: 100%; overflow: hidden; background: var(--background); }
.viewer-empty { display: flex; align-items: center; justify-content: center; flex: 1; color: var(--muted-foreground); font-size: 12px; }
.viewer-tabs { display: flex; align-items: center; gap: 2px; flex: none; padding: 4px 6px 0; border-bottom: 1px solid var(--border); background: var(--muted); overflow-x: auto; overflow-y: hidden; white-space: nowrap; }
.viewer-tabs::-webkit-scrollbar { height: 4px; }
.viewer-tabs::-webkit-scrollbar-thumb { border-radius: 2px; background: var(--border); }
.viewer-tab { display: inline-flex; align-items: center; gap: 4px; flex: none; max-width: 160px; padding: 3px 6px 3px 8px; border: 1px solid transparent; border-bottom: none; border-radius: 4px 4px 0 0; color: var(--foreground); font-size: 11px; cursor: pointer; }
.viewer-tab:hover { background: var(--accent); }
.viewer-tab.active { border-color: var(--border); background: var(--background); color: var(--primary); }
.tab-icon { flex: none; display: block; width: 13px; height: 13px; }
.tab-name { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tab-dot { flex: none; width: 6px; height: 6px; border-radius: 50%; background: #e6a23c; }
.viewer-tab.dirty .tab-name { color: #e6a23c; }
.tab-close { flex: none; width: 14px; height: 14px; line-height: 12px; text-align: center; border-radius: 3px; color: var(--muted-foreground); font-size: 12px; }
.tab-close:hover { background: var(--accent); color: var(--destructive); }
.viewer-body { flex: 1; min-height: 0; }
</style>
