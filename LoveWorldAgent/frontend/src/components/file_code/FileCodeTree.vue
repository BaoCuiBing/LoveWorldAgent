<template>
    <div class="file-tree">
        <div class="tree-bar">
            <button class="tree-refresh" type="button" aria-label="刷新文件列表" @click="emit('refresh')">
                <RefreshCw :size="14" />
            </button>
        </div>
        <div class="tree-body" ref="treeBodyEl" @contextmenu.prevent="onBlank($event)">
            <div v-if="rootName" class="tree-row tree-root" @click="toggleRoot" @contextmenu.prevent.stop="onRootMenu($event)">
                <ChevronRight class="tree-arrow" :class="{ open: rootOpen }" :size="12" />
                <img class="tree-icon" :src="iconFolder" alt="" />
                <span class="tree-name tree-root-name" :title="rootName">{{ rootName }}</span>
            </div>
            <div v-for="row in rows" :key="row.rel" class="tree-row" :class="{ active: row.type === 'file' && row.rel === activeRel }" :style="{ paddingLeft: `${8 + (row.depth + (rootName ? 1 : 0)) * 14}px` }" @click="onClickRow(row)" @contextmenu.prevent.stop="onRow(row, $event)">
                <ChevronRight v-if="row.expandable" class="tree-arrow" :class="{ open: row.open }" :size="12" />
                <span v-else class="tree-arrow-placeholder"></span>
                <img class="tree-icon" :src="iconOf(row)" alt="" />
                <span class="tree-name" :class="toneOf(row)" :title="row.error || row.rel">{{ row.name }}</span>
                <LoaderCircle v-if="row.loading" class="animate-spin tree-state" :size="12" />
                <span v-else-if="row.error" class="tree-state tree-state-error" :title="row.error">!</span>
            </div>
            <div v-if="!rootName && !rows.length" class="tree-empty">暂无可展示的文件</div>
        </div>
    </div>
</template>

<script setup>
import { computed, nextTick, reactive, ref, watch } from 'vue'
import { ChevronRight, LoaderCircle, RefreshCw } from 'lucide-vue-next'
import { projectFiles } from '../../api/request_data'
import { fileIconOf, toneOf as toneOfExt, ICON_FOLDER as iconFolder } from '../../assets/fileIcons'
const props = defineProps({
    items: { type: Array, default: () => [] },
    activeRel: { type: String, default: '' },
    rootName: { type: String, default: '' },
})
const emit = defineEmits(['open', 'refresh', 'menu'])
const iconOf = (row) => fileIconOf(row && row.name, row && row.type)
const toneOf = (row) => toneOfExt(row && row.name, row && row.type)
// 目录节点按需解析（参考 VS Code ExplorerItem 的 _isDirectoryResolved / fetchChildren / forgetChildren）：
// /api/project/files 只返回一层，子项在展开时单独请求，否则「展开后一片空白」
const nodes = reactive(new Map())  // rel -> { rel, name, type, size, hasChildren, children[rel], resolved, loading, error }
const roots = ref([])
const treeBodyEl = ref(null)
const expanded = reactive(new Set())
const rootOpen = ref(true)
const baseName = (rel) => String(rel || '').split('/').pop() || rel
const seed = (list, parentRel) => {
    const rels = []
    ;(list || []).forEach((item) => {
        const rel = String((item && item.rel) || '').trim()
        if (!rel) return
        rels.push(rel)
        const type = (item && item.type) === 'dir' ? 'dir' : 'file'
        const hasChildren = type === 'dir' && Boolean(item && item.has_children)
        const cached = nodes.get(rel)
        if (cached) {
            cached.name = (item && item.name) || baseName(rel)
            cached.type = type
            cached.size = Number((item && item.size) || 0)
            cached.hasChildren = hasChildren
            cached.resolved = type === 'dir' ? !hasChildren : true
            cached.error = ''
            if (type !== 'dir') cached.children = []
            return
        }
        nodes.set(rel, { rel, name: (item && item.name) || baseName(rel), type, size: Number((item && item.size) || 0), hasChildren, children: [], resolved: type === 'dir' ? !hasChildren : true, loading: false, error: '' })
    })
    if (!parentRel) {
        roots.value = rels
        return
    }
    const parent = nodes.get(parentRel)
    if (parent) {
        parent.children = rels
        parent.resolved = true
    }
}
const resolveDir = async (rel) => {
    const node = nodes.get(rel)
    if (!node || node.type !== 'dir' || node.loading) return
    node.loading = true
    node.error = ''
    try {
        const data = await projectFiles(rel)
        seed(Array.isArray(data && data.list) ? data.list : [], rel)
    } catch (err) {
        node.error = (err && err.message) || String(err)  // 失败挂在节点上，展开箭头仍在，再点可重试
    } finally {
        node.loading = false
    }
}
const ensureDir = async (rel) => {
    const node = nodes.get(rel)
    if (!node || node.type !== 'dir' || node.resolved) return
    await resolveDir(rel)
}
const forgetAll = () => {
    nodes.clear()
}
// 刷新后重建根层，并把仍处展开态的目录逐层重新解析（深度按父→子推进）
const restoreExpanded = async (rels) => {
    await Promise.all(rels.map(async (rel) => {
        if (!expanded.has(rel)) return
        const node = nodes.get(rel)
        if (!node || node.type !== 'dir') return
        await ensureDir(rel)
        await restoreExpanded(node.children)
    }))
}
const rows = computed(() => {
    if (props.rootName && !rootOpen.value) return []
    const out = []
    const walk = (rels, depth) => {
        rels.forEach((rel) => {
            const node = nodes.get(rel)
            if (!node) return
            const open = expanded.has(node.rel)
            out.push({ rel: node.rel, name: node.name, type: node.type, size: node.size, depth, open, expandable: node.type === 'dir' && node.hasChildren, loading: node.loading, error: node.error })
            if (node.type === 'dir' && open) walk(node.children, depth + 1)
        })
    }
    walk(roots.value, 0)
    return out
})
// 外部切文件时逐级展开父目录，并在节点就绪后选中、滚动到目标行
const revealActive = async (rel) => {
    const parts = String(rel || '').split('/')
    for (let index = 1; index < parts.length; index += 1) {
        const dir = parts.slice(0, index).join('/')
        const node = nodes.get(dir)
        if (!node || node.type !== 'dir') return
        expanded.add(dir)
        await ensureDir(dir)
    }
    await nextTick()
    const el = treeBodyEl.value && treeBodyEl.value.querySelector('.tree-row.active')
    if (el) el.scrollIntoView({ block: 'nearest' })
}
watch(() => props.items, async (list) => {
    const keep = new Set(expanded)
    forgetAll()
    expanded.clear()
    seed(list, '')
    keep.forEach((rel) => expanded.add(rel))
    await restoreExpanded(roots.value)
    if (props.activeRel) await revealActive(props.activeRel)
}, { immediate: true })
watch(() => props.activeRel, async (rel) => {
    await revealActive(rel)
})
const toggleRoot = async () => {
    rootOpen.value = !rootOpen.value
    if (!rootOpen.value) {
        expanded.clear()
        return
    }
    roots.value.forEach((rel) => {
        const node = nodes.get(rel)
        if (node && node.type === 'dir') expanded.add(rel)
    })
    await restoreExpanded(roots.value)
}
const toggleDir = (rel) => {
    if (expanded.has(rel)) {
        expanded.delete(rel)
        return
    }
    expanded.add(rel)
    ensureDir(rel)
}
const onClickRow = (row) => {
    if (row.type === 'dir') {
        if (row.expandable) toggleDir(row.rel)
        return
    }
    emit('open', row.rel)
}
const onRow = (row, event) => {
    emit('menu', { rel: row.rel, type: row.type, x: event.clientX, y: event.clientY })
}
const onRootMenu = (event) => {
    emit('menu', { rel: '', type: 'dir', x: event.clientX, y: event.clientY })
}
const onBlank = (event) => {
    emit('menu', { rel: '', type: 'dir', x: event.clientX, y: event.clientY })
}
</script>

<style scoped>
.file-tree { display: flex; flex-direction: column; min-height: 0; height: 100%; }
.tree-bar { display: flex; align-items: center; gap: 6px; padding: 0 4px 6px; }
.tree-refresh { display: inline-flex; align-items: center; justify-content: center; flex: none; width: 24px; height: 24px; padding: 0; border: none; border-radius: 4px; background: transparent; color: var(--muted-foreground); cursor: pointer; }
.tree-refresh:hover { color: var(--primary); background: var(--accent); }
.tree-body { flex: 1; min-height: 0; overflow: auto; padding: 2px 4px; }
.tree-row { display: flex; align-items: center; gap: 4px; height: 26px; padding-right: 6px; border-radius: 4px; cursor: pointer; }
.tree-row:hover { background: var(--accent); }
.tree-row.active { background: color-mix(in srgb, var(--primary) 12%, transparent); color: var(--primary); }
.tree-root { padding-left: 8px; }
.tree-arrow, .tree-arrow-placeholder { flex: none; width: 12px; color: var(--muted-foreground); transition: transform 0.15s ease; }
.tree-arrow.open { transform: rotate(90deg); }
.tree-arrow-placeholder { display: inline-block; width: 12px; height: 12px; }
.tree-icon { flex: none; width: 14px; height: 14px; object-fit: contain; }
.tree-name { flex: 1; min-width: 0; overflow: hidden; font-size: 12px; text-overflow: ellipsis; white-space: nowrap; color: var(--foreground); }
.tree-root-name { font-weight: 600; }
.tree-row.active .tree-name { color: var(--primary); }
.tree-state { flex: none; width: 12px; color: var(--primary); }
.tree-state-error { color: var(--destructive); font-weight: 700; text-align: center; }
.tree-name.tone-blue { color: #4A9EFF; }
.tree-name.tone-purple { color: #A98BFF; }
.tree-name.tone-yellow { color: #D4B106; }
.tree-name.tone-orange { color: #E2743C; }
.tree-name.tone-green { color: #4CAF50; }
.tree-empty { padding: 12px 6px; font-size: 12px; color: var(--muted-foreground); }
</style>
