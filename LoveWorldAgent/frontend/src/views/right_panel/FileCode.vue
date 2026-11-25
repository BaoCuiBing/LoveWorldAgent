<template>
    <div ref="rootEl" class="file-code">
        <div class="fc-body">
            <div class="fc-preview">
                <FileCodeViewer :tabs="tabs" :active-rel="activeRel" :versions="versions" @activate="activateTab" @close="closeTab" @save="onSave" />
            </div>
            <div class="fc-splitter" role="separator" aria-orientation="vertical" tabindex="0" :aria-valuenow="Math.round(treeWidth)" aria-valuemin="20" aria-valuemax="70" @pointerdown="onPointerDown" @keydown="onKeyDown"></div>
            <div class="fc-tree" :style="{ width: `${treeWidth}%` }">
                <FileCodeTree :items="items" :root-name="rootName" :active-rel="activeRel" @open="openFile" @refresh="loadFiles(false)" @menu="openMenu" />
            </div>
        </div>
        <Teleport to="body">
            <div ref="menuEl" v-if="menu.visible" class="fc-menu" :style="{ left: `${menu.x}px`, top: `${menu.y}px` }" tabindex="-1" @pointerdown.stop>
                <template v-for="item in menuItems" :key="item.key">
                    <div v-if="item.separator" class="fc-menu-sep" />
                    <button v-else type="button" class="fc-menu-item" :class="{ danger: item.danger, disabled: item.disabled }" :disabled="item.disabled" @click="onMenuCommand(item)">{{ item.label }}</button>
                </template>
            </div>
        </Teleport>
        <Dialog v-model:open="createOpen">
            <DialogContent class="max-w-sm">
                <DialogHeader>
                    <DialogTitle>{{ createType === 'file' ? '创建文件' : '创建文件夹' }}</DialogTitle>
                    <DialogDescription>{{ createType === 'file' ? '新建文件名（含后缀）' : '新建文件夹名称' }}</DialogDescription>
                </DialogHeader>
                <Input v-model="createName" :placeholder="createType === 'file' ? '例如 note.md' : '例如 assets'" @keyup.enter="submitCreate" />
                <div v-if="createError" class="create-error">{{ createError }}</div>
                <DialogFooter>
                    <Button variant="outline" size="sm" @click="createOpen = false">取消</Button>
                    <Button size="sm" :disabled="createSaving" @click="submitCreate">确定</Button>
                </DialogFooter>
            </DialogContent>
        </Dialog>
    </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import { Button } from '../../components/shadcn_ui/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '../../components/shadcn_ui/ui/dialog'
import { Input } from '../../components/shadcn_ui/ui/input'
import { toast } from '../../components/shadcn_ui/lib/useToast'
import { confirm } from '../../components/shadcn_ui/lib/useConfirm'
import { copyProjectEntry, createProjectEntry, deleteProjectEntry, projectFile, projectFiles, saveProjectFile } from '../../api/request_data'
import FileCodeTree from '../../components/file_code/FileCodeTree.vue'
import FileCodeViewer from '../../components/file_code/FileCodeViewer.vue'
const props = defineProps({ projectPath: { type: String, default: '' } })
const NAME_RE = /^[^\\/:*?"<>|]+$/
// 图片/Office/PDF 交给 OfficeView 按字节流渲染，不走 /api/project/file 的文本读取
const BINARY_EXTS = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg', 'ico', 'pdf', 'doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx']
const isBinary = (rel) => BINARY_EXTS.includes(String(rel || '').split('.').pop().toLowerCase())
const rootEl = ref(null)
const menuEl = ref(null)
const treeWidth = ref(23)  // 默认右侧树占 23%（编辑器 77%），与参考布局一致；拖动区间仍 20%~70%
const items = ref([])
const tabs = ref([])
const versions = reactive({})
const activeRel = ref('')
const clipboard = ref('')
const menu = reactive({ visible: false, x: 0, y: 0, rel: '', type: 'dir' })
const createOpen = ref(false)
const createType = ref('file')
const createDir = ref('')
const createName = ref('')
const createError = ref('')
const createSaving = ref(false)
let stopDrag = null
const baseOf = (rel) => String(rel || '').split('/').pop()
const parentOf = (rel) => { const index = String(rel || '').lastIndexOf('/'); return index === -1 ? '' : String(rel).slice(0, index) }
const targetDir = computed(() => (menu.type === 'dir' ? menu.rel : parentOf(menu.rel)))
const rootName = computed(() => {
    const base = String(props.projectPath || '').replace(/[\\/]+$/, '')
    return base ? base.split(/[\\/]/).pop() : ''
})
const joinRel = (dir, name) => (dir ? `${dir}/${name}` : name)
const onPointerDown = (event) => {
    if (stopDrag) return
    const rect = rootEl.value && rootEl.value.getBoundingClientRect()
    if (!rect || !rect.width) return
    event.preventDefault()
    let frame = 0
    let pendingX = event.clientX
    const apply = () => {
        frame = 0
        const pct = ((rect.right - pendingX) / rect.width) * 100
        treeWidth.value = Math.min(70, Math.max(20, pct))
    }
    const move = (e) => { pendingX = e.clientX; if (!frame) frame = window.requestAnimationFrame(apply) }
    const up = () => {
        if (frame) window.cancelAnimationFrame(frame)
        window.removeEventListener('pointermove', move)
        window.removeEventListener('pointerup', up)
        document.body.style.userSelect = ''
        document.body.style.cursor = ''
        stopDrag = null
    }
    document.body.style.userSelect = 'none'
    document.body.style.cursor = 'col-resize'
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up)
    stopDrag = up
}
const onKeyDown = (event) => {
    if (event.key === 'ArrowLeft') treeWidth.value = Math.min(70, treeWidth.value + 2)
    else if (event.key === 'ArrowRight') treeWidth.value = Math.max(20, treeWidth.value - 2)
    else return
    event.preventDefault()
}
const loadFiles = async (silent = true) => {
    try {
        const data = await projectFiles() // path 恒为空：始终列项目根，由后端按 app.project_path 解析
        items.value = Array.isArray(data && data.list) ? data.list : []
    } catch (err) {
        if (!silent) toast.warning(`读取文件列表失败：${err.message}`)
    }
}
const activateTab = (rel) => {
    if (tabs.value.some((tab) => tab.rel === rel)) activeRel.value = rel
}
const openFile = async (rel, refresh = false) => {
    if (!rel) return
    if (refresh) versions[rel] = (versions[rel] || 0) + 1
    if (tabs.value.some((tab) => tab.rel === rel)) { activateTab(rel); return }
    const binary = isBinary(rel)
    tabs.value.push({ rel, name: baseOf(rel), content: '', saved: '', loading: !binary, binary })
    activeRel.value = rel
    if (binary) return  // OfficeView 自行通过 /api/project/file/raw 取字节流
    try {
        const data = await projectFile(rel)
        const tab = tabs.value.find((item) => item.rel === rel)
        if (!tab) return
        tab.content = String((data && data.content) || '')
        tab.saved = tab.content
        tab.loading = false
    } catch (err) {
        toast.error(`读取文件失败：${err.message}`)
        dropTab(rel, false)
    }
}
const dropTab = (rel, isDir) => {
    tabs.value = tabs.value.filter((tab) => (isDir ? tab.rel !== rel && !tab.rel.startsWith(`${rel}/`) : tab.rel !== rel))
    if (activeRel.value && !tabs.value.some((tab) => tab.rel === activeRel.value)) activeRel.value = tabs.value.length ? tabs.value[tabs.value.length - 1].rel : ''
}
const closeTab = async (rel) => {
    const hit = tabs.value.find((tab) => tab.rel === rel)
    if (!hit) return
    if (hit.content !== hit.saved) {
        const ok = await confirm({ title: '未保存的修改', description: `「${hit.name}」有未保存的修改，关闭后将丢失，确定继续？`, confirmText: '关闭', destructive: true })
        if (!ok) return
    }
    const index = tabs.value.findIndex((tab) => tab.rel === rel)
    if (index === -1) return
    tabs.value.splice(index, 1)
    if (activeRel.value === rel) activeRel.value = tabs.value.length ? tabs.value[Math.min(index, tabs.value.length - 1)].rel : ''
}
const onSave = async ({ rel, text }) => {
    const tab = tabs.value.find((item) => item.rel === rel)
    if (!tab) return
    try {
        await saveProjectFile(rel, text)
        tab.content = text // 回写服务端原文，脏标记随之清除
        tab.saved = text
        toast.success('已保存')
    } catch (err) {
        toast.error(`保存失败：${err.message}`)
    }
}
const openMenu = (payload) => {
    menu.visible = true
    menu.x = payload.x
    menu.y = payload.y
    menu.rel = payload.rel
    menu.type = payload.type
    nextTick(() => {
        const el = menuEl.value
        if (!el) return
        const margin = 8
        const width = el.offsetWidth || 140
        const height = el.offsetHeight || 240
        menu.x = Math.max(margin, Math.min(menu.x, window.innerWidth - width - margin))
        menu.y = Math.max(margin, Math.min(menu.y, window.innerHeight - height - margin))
    })
}
const closeMenu = () => { menu.visible = false }
const absolutePath = (rel) => {
    const base = String(props.projectPath || '').replace(/[\\/]+$/, '')
    if (!base || !rel) return ''
    return `${base}\\${String(rel).replace(/\//g, '\\')}`
}
const writeClipboard = async (value) => {
    try {
        if (navigator.clipboard && navigator.clipboard.writeText) { await navigator.clipboard.writeText(value); return true }
    } catch { /* 回退到 execCommand */ }
    try {
        const el = document.createElement('textarea')
        el.value = value
        el.style.position = 'fixed'
        el.style.opacity = '0'
        document.body.appendChild(el)
        el.select()
        const ok = document.execCommand('copy')
        document.body.removeChild(el)
        return ok
    } catch { return false }
}
const menuItems = computed(() => {
    const list = [{ key: 'refresh', label: '刷新' }, { key: 'sep1', separator: true }]
    if (menu.type === 'dir') {
        list.push({ key: 'newFile', label: '创建文件' })
        list.push({ key: 'newDir', label: '创建文件夹' })
    }
    list.push({ key: 'delete', label: '删除', danger: true, disabled: !menu.rel })
    list.push({ key: 'copy', label: '复制', disabled: !menu.rel })
    list.push({ key: 'paste', label: '粘贴', disabled: !clipboard.value })
    list.push({ key: 'sep2', separator: true })
    list.push({ key: 'addToChat', label: '添加到对话', disabled: !menu.rel })
    list.push({ key: 'copyPath', label: '复制文件路径', disabled: !menu.rel || !props.projectPath })
    return list
})
// 新建文件/文件夹：用 shadcn Dialog + Input 替代 Element 的 prompt 弹窗，校验规则与文案保持原样
const openCreate = (dir, type) => {
    createType.value = type
    createDir.value = dir
    createName.value = ''
    createError.value = ''
    createOpen.value = true
}
const submitCreate = async () => {
    if (createSaving.value) return
    const isFile = createType.value === 'file'
    const value = String(createName.value || '').trim()
    if (!value) { createError.value = '名称不能为空'; return }
    if (!NAME_RE.test(value)) { createError.value = '名称不能包含 \\ / : * ? " < > |'; return }
    if (value === '.' || value === '..' || value.startsWith('.')) { createError.value = '不支持以点开头的名称'; return }
    if (isFile && !value.includes('.')) { createError.value = '文件必须带后缀'; return }
    createError.value = ''
    const rel = joinRel(createDir.value, value)
    createSaving.value = true
    try {
        await createProjectEntry(rel, createType.value)
        toast.success(`已创建 ${value}`)
        createOpen.value = false
        await loadFiles(true)
    } catch (err) {
        toast.error(`创建失败：${err.message}`) // 409 时后端文案即「同名已存在」
    } finally {
        createSaving.value = false
    }
}
const removeEntry = async (rel, type) => {
    if (!rel) return
    const isDir = type === 'dir'
    const ok = await confirm({
        title: '删除',
        description: isDir ? `删除文件夹「${baseOf(rel)}」会递归删除其下全部内容，确定吗？` : `删除文件「${baseOf(rel)}」？`,
        confirmText: '删除',
        destructive: true,
    })
    if (!ok) return
    try {
        const data = await deleteProjectEntry(rel)
        toast.success(isDir ? `已删除（含 ${Number((data && data.removed) || 0)} 项）` : '已删除')
        dropTab(rel, isDir)
        if (clipboard.value === rel) clipboard.value = ''
        await loadFiles(true)
    } catch (err) {
        toast.error(`删除失败：${err.message}`)
    }
}
const pasteEntry = async (dir) => {
    const src = clipboard.value
    if (!src) return
    try {
        const data = await copyProjectEntry(src, joinRel(dir, baseOf(src)))
        const finalName = baseOf(String((data && data.path) || ''))
        toast.success(data && data.renamed ? `已粘贴为 ${finalName}` : '已粘贴')
        await loadFiles(true)
    } catch (err) {
        toast.error(`粘贴失败：${err.message}`)
    }
}
const onMenuCommand = async (item) => {
    if (item.separator) return
    closeMenu()
    if (item.disabled) return
    const dir = targetDir.value
    if (item.key === 'refresh') { loadFiles(false); return }
    if (item.key === 'newFile') { openCreate(dir, 'file'); return }
    if (item.key === 'newDir') { openCreate(dir, 'dir'); return }
    if (item.key === 'delete') { removeEntry(menu.rel, menu.type); return }
    if (item.key === 'addToChat') {
        if (!menu.rel) return
        window.dispatchEvent(new CustomEvent('lwa:add-file-to-chat', { detail: { rel: menu.rel, type: menu.type } }))
        toast.success('已添加到对话')
        return
    }
    if (item.key === 'copyPath') {
        const value = absolutePath(menu.rel)
        if (!value) return
        const ok = await writeClipboard(value)
        if (ok) toast.success('已复制文件路径')
        else toast.error('复制失败')
        return
    }
    if (item.key === 'copy') {
        clipboard.value = menu.rel
        toast.success(`已复制 ${baseOf(menu.rel)}`)
    } else if (item.key === 'paste') {
        pasteEntry(dir)
    }
}
const onDocPointerDown = (event) => {
    if (!menu.visible) return
    const target = event.target
    if (target && typeof target.closest === 'function' && target.closest('.fc-menu')) return
    closeMenu()
}
const onDocKeyDown = (event) => { if (event.key === 'Escape') closeMenu() }
watch(() => props.projectPath, () => {
    clipboard.value = ''
    tabs.value = []
    activeRel.value = ''
    loadFiles(true)
})
onMounted(() => {
    loadFiles(true)
    window.addEventListener('pointerdown', onDocPointerDown, true)
    window.addEventListener('keydown', onDocKeyDown)
})
onBeforeUnmount(() => {
    if (stopDrag) stopDrag()
    window.removeEventListener('pointerdown', onDocPointerDown, true)
    window.removeEventListener('keydown', onDocKeyDown)
})
defineExpose({ openFile })
</script>

<style scoped>
.file-code { display: flex; flex-direction: column; min-height: 0; height: 100%; }
.fc-body { flex: 1; min-height: 0; display: flex; }
.fc-preview { flex: 1; min-width: 0; min-height: 0; }
.fc-splitter { flex: none; width: 12px; margin: 0; background: transparent; position: relative; cursor: col-resize; }
.fc-splitter::before { content: ''; position: absolute; top: 0; bottom: 0; left: 50%; width: 1px; background: var(--border); transform: translateX(-50%); transition: background 0.15s ease; }
.fc-splitter:hover::before, .fc-splitter:focus-visible::before { width: 2px; background: var(--primary); }
.fc-splitter:focus-visible { outline: none; }
.fc-tree { flex: none; min-width: 0; min-height: 0; }
.create-error { font-size: 11px; color: var(--destructive); }
</style>

<style>
.fc-menu { position: fixed; z-index: 3000; min-width: 132px; padding: 4px; border: 1px solid var(--border); border-radius: 6px; background: var(--popover); color: var(--popover-foreground); box-shadow: 0 6px 16px rgba(0, 0, 0, .12); font-size: 12px; }
.fc-menu-item { display: block; width: 100%; padding: 5px 8px; border: none; border-radius: 4px; background: transparent; color: var(--foreground); text-align: left; cursor: pointer; font-size: 12px; }
.fc-menu-item:hover { background: var(--accent); }
.fc-menu-item.danger { color: var(--destructive); }
.fc-menu-item.disabled, .fc-menu-item:disabled { color: var(--muted-foreground); cursor: not-allowed; background: transparent; }
.fc-menu-sep { height: 1px; margin: 4px 6px; background: var(--border); }
</style>
