<template>
    <div class="office-view">
        <div v-if="kind === 'unsupported'" class="office-placeholder">
            <FileText :size="36" />
            <div class="office-title">{{ ext.toUpperCase() }} 暂不支持预览</div>
            <div class="office-desc">旧版 Office 格式无可靠的前端渲染库，请转存为 {{ hint }} 后重新打开</div>
        </div>
        <div v-else-if="error" class="office-placeholder">
            <TriangleAlert :size="36" />
            <div class="office-title">预览失败</div>
            <div class="office-desc">{{ error }}</div>
        </div>
        <template v-else>
            <div v-if="kind === 'xls' && sheetNames.length > 1" class="office-sheetbar">
                <button v-for="sheet in sheetNames" :key="sheet" type="button" class="office-sheet" :class="{ active: sheet === activeSheet }" @click="selectSheet(sheet)">{{ sheet }}</button>
            </div>
            <div class="office-stage">
                <div v-if="loading" class="office-placeholder office-loading"><LoaderCircle :size="28" class="animate-spin" /><div class="office-desc">正在加载 {{ name }}…</div></div>
                <div ref="host" class="office-host" :class="`office-${kind}`"></div>
            </div>
            <div v-if="kind === 'pdf' && pageCount > 1" class="office-pagebar">共 {{ pageCount }} 页 · 向下滚动查看全部</div>
        </template>
    </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { FileText, LoaderCircle, TriangleAlert } from 'lucide-vue-next'
import { projectFileRaw } from '../../api/request_data'
const props = defineProps({ rel: { type: String, default: '' }, version: { type: Number, default: 0 } })
const host = ref(null)
const loading = ref(false)
const error = ref('')
const name = ref('')
const sheetNames = ref([])
const activeSheet = ref('')
const pageCount = ref(0)
const MAX_XLSX_ROWS = 5000  // 大表截断，避免一次性生成几十万行 DOM
let pdfDoc = null
let pptxViewer = null
let workbook = null
let XLSX = null
let blobUrls = []
let renderToken = 0
const ext = computed(() => (String(props.rel || '').split('.').pop() || '').toLowerCase())
const IMAGE_EXTS = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg', 'ico']
const kind = computed(() => {
    const value = ext.value
    if (IMAGE_EXTS.includes(value)) return 'image'
    if (value === 'pdf') return 'pdf'
    if (value === 'docx') return 'docx'
    if (value === 'xls' || value === 'xlsx') return 'xls'
    if (value === 'pptx') return 'pptx'
    return 'unsupported'
})
const hint = computed(() => ({ doc: 'DOCX', ppt: 'PPTX' }[ext.value] || 'DOCX / XLSX / PPTX'))
const dispose = () => {
    if (host.value) host.value.innerHTML = ''
    blobUrls.forEach((url) => URL.revokeObjectURL(url))
    blobUrls = []
    if (pdfDoc) { pdfDoc.destroy(); pdfDoc = null }
    if (pptxViewer) { pptxViewer.destroy(); pptxViewer = null }
    workbook = null
}
const renderImage = (buffer) => {
    const url = URL.createObjectURL(new Blob([buffer]))
    blobUrls.push(url)
    const img = document.createElement('img')
    img.className = 'office-image'
    img.alt = name.value
    img.src = url
    host.value.appendChild(img)
}
const renderDocx = async (buffer) => {
    const { renderAsync } = await import('docx-preview')
    await renderAsync(buffer, host.value, host.value, { className: 'office-docx', inWrapper: true, ignoreWidth: false, ignoreHeight: true, breakPages: false })
}
const escapeHtml = (value) => String(value == null ? '' : value).replace(/[&<>"']/g, (ch) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[ch]))
const sheetHtml = () => {
    const ws = workbook.Sheets[activeSheet.value]
    const rows = XLSX.utils.sheet_to_json(ws, { header: 1, raw: false, defval: '', blankrows: false })
    const limited = rows.length > MAX_XLSX_ROWS
    const visible = limited ? rows.slice(0, MAX_XLSX_ROWS) : rows
    const body = visible.map((row) => `<tr>${row.map((cell) => `<td>${escapeHtml(cell)}</td>`).join('')}</tr>`).join('')
    host.value.innerHTML = `<table class="office-xls-table"><tbody>${body}</tbody></table>${limited ? `<div class="office-truncated">表格过大，仅显示前 ${MAX_XLSX_ROWS} 行</div>` : ''}`
}
const renderXls = (buffer) => {
    workbook = XLSX.read(new Uint8Array(buffer), { type: 'array', cellStyles: true })
    sheetNames.value = workbook.SheetNames || []
    activeSheet.value = sheetNames.value[0] || ''
    sheetHtml()
}
const selectSheet = (value) => { activeSheet.value = value; sheetHtml() }
const renderPptx = async (buffer) => {
    const { init } = await import('pptx-preview')
    const width = Math.max(640, Math.floor(host.value.clientWidth || 800))
    pptxViewer = init(host.value, { width, height: Math.round(width * 9 / 16), mode: 'slide' })
    await pptxViewer.preview(buffer)
}
const renderPdf = async (buffer) => {
    const pdfjs = await import('pdfjs-dist')
    if (!pdfjs.GlobalWorkerOptions.workerSrc) {
        const workerText = (await import('pdfjs-dist/build/pdf.worker.min.mjs?raw')).default
        pdfjs.GlobalWorkerOptions.workerSrc = URL.createObjectURL(new Blob([workerText], { type: 'text/javascript' }))
    }
    pdfDoc = await pdfjs.getDocument({ data: new Uint8Array(buffer) }).promise
    pageCount.value = pdfDoc.numPages
    const container = host.value
    for (let index = 1; index <= pdfDoc.numPages; index += 1) {
        const page = await pdfDoc.getPage(index)
        const scale = Math.min(2, Math.max(1, (container.clientWidth - 32) / page.getViewport({ scale: 1 }).width))
        const viewport = page.getViewport({ scale })
        const canvas = document.createElement('canvas')
        canvas.className = 'office-pdf-page'
        canvas.width = Math.floor(viewport.width)
        canvas.height = Math.floor(viewport.height)
        container.appendChild(canvas)
        await page.render({ canvasContext: canvas.getContext('2d'), viewport }).promise
        page.cleanup()
    }
}
const render = async () => {
    const token = ++renderToken
    dispose()
    sheetNames.value = []
    pageCount.value = 0
    error.value = ''
    name.value = String(props.rel || '').split('/').pop()
    if (!props.rel || kind.value === 'unsupported') return
    loading.value = true
    await nextTick()
    try {
        const buffer = await projectFileRaw(props.rel)
        if (token !== renderToken || !host.value) return
        if (kind.value === 'image') renderImage(buffer)
        else if (kind.value === 'pdf') await renderPdf(buffer)
        else if (kind.value === 'docx') await renderDocx(buffer)
        else if (kind.value === 'xls') {
            if (!XLSX) XLSX = await import('xlsx')
            renderXls(buffer)
        } else if (kind.value === 'pptx') await renderPptx(buffer)
    } catch (err) {
        if (token === renderToken) error.value = String((err && err.message) || err)
    } finally {
        if (token === renderToken) loading.value = false
    }
}
watch([() => props.rel, () => props.version], render, { immediate: true })
onBeforeUnmount(() => { renderToken += 1; dispose() })
</script>

<style scoped>
.office-view { display: flex; flex-direction: column; height: 100%; min-height: 0; background: var(--background); }
.office-stage { position: relative; flex: 1; min-height: 0; }
.office-host { position: absolute; inset: 0; overflow: auto; }
.office-placeholder { display: flex; flex: 1; flex-direction: column; align-items: center; justify-content: center; gap: 8px; color: var(--muted-foreground); text-align: center; }
.office-loading { position: absolute; inset: 0; z-index: 1; background: var(--background); }
.office-title { font-size: 13px; font-weight: 600; color: var(--foreground); }
.office-desc { max-width: 420px; font-size: 12px; line-height: 1.7; }
.office-sheetbar { display: flex; flex: none; gap: 2px; padding: 4px 8px; overflow-x: auto; border-bottom: 1px solid var(--border); }
.office-sheet { flex: none; padding: 3px 10px; border: none; border-radius: 4px; background: transparent; color: var(--muted-foreground); font-size: 11px; cursor: pointer; }
.office-sheet:hover { background: var(--accent); }
.office-sheet.active { background: color-mix(in srgb, var(--primary) 12%, transparent); color: var(--primary); }
.office-pagebar { flex: none; padding: 4px 10px; border-top: 1px solid var(--border); color: var(--muted-foreground); font-size: 11px; text-align: center; }
.office-host :deep(.office-image) { display: block; max-width: 100%; max-height: 100%; margin: auto; object-fit: contain; }
.office-host :deep(.office-pdf-page) { display: block; margin: 12px auto; border-radius: 2px; box-shadow: 0 2px 10px rgba(0, 0, 0, .35); }
.office-host :deep(.docx-wrapper) { background: #525252; }
.office-host :deep(.docx-wrapper > section.docx) { margin: 12px auto; box-shadow: 0 2px 10px rgba(0, 0, 0, .35); }
.office-host :deep(.office-xls-table) { border-collapse: collapse; font-size: 12px; }
.office-host :deep(.office-xls-table td) { padding: 3px 8px; border: 1px solid var(--border); white-space: nowrap; }
.office-host :deep(.office-truncated) { padding: 8px; color: #e6a23c; font-size: 12px; }
.office-pptx { display: flex; align-items: flex-start; justify-content: center; }
</style>
