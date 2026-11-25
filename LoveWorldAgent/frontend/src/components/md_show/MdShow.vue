<template>
    <div ref="rootRef" class="md-show-wrap">
        <template v-for="seg in segments">
            <div v-if="seg.type === 'text'" :key="`${seg.key}-text`" class="md-show" v-html="renderText(seg.text)"></div>
            <div v-else-if="seg.type === 'code'" :key="`${seg.key}-code`" class="md-code">
                <div class="md-code-head"><span class="md-code-lang">{{ seg.lang || 'text' }}</span><span class="md-code-actions"><button class="md-copy-btn" type="button" aria-label="复制代码" @click="copyCode(seg.code)">复制</button></span></div>
                <CodeEditor :model-value="seg.code" :language="codeLang(seg.lang)" read-only :lines="seg.code.split('\n').length" />
            </div>
            <div v-else-if="mermaidFailed.has(seg.key)" :key="`${seg.key}-failed`" class="md-code md-mermaid-fallback">
                <div class="md-code-head"><span class="md-code-lang">mermaid（图形渲染失败）</span><span class="md-code-actions"><button class="md-copy-btn" type="button" aria-label="复制代码" @click="copyCode(seg.code)">复制</button></span></div>
                <CodeEditor :model-value="seg.code" language="plaintext" read-only :lines="seg.code.split('\n').length" />
            </div>
            <div v-else :key="`${seg.key}-mermaid-${themeTick}`" class="md-code md-mermaid" :data-mermaid-key="seg.key">
                <div class="md-code-head"><span class="md-code-lang">mermaid</span><span class="md-code-actions"><button class="md-copy-btn" type="button" aria-label="复制代码" @click="copyCode(seg.code)">复制</button></span></div>
                <pre class="md-mermaid-source"><code>{{ seg.code }}</code></pre>
            </div>
        </template>
    </div>
    <div v-if="streaming && (!content || hasStreamingFence)" class="md-generating"><span class="md-generating-dot" />生成中…</div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import MarkdownIt from 'markdown-it'
import { toast } from '@/components/shadcn_ui/lib/useToast'
import CodeEditor from '../editor/CodeEditor.vue'
import { codeLang } from './toolPart'
const props = defineProps({ content: { type: String, default: '' }, streaming: { type: Boolean, default: false } })
const md = new MarkdownIt({ html: false, breaks: true, linkify: true })
const escapeHtml = md.utils.escapeHtml
// 链接协议白名单：只允许 http/https/mailto/file/锚点/相对路径，拦截 javascript:、vbscript:、data:
const ALLOWED_PROTOCOL = /^(https?:|mailto:|file:|#|\/|\.{1,2}\/)/i
md.validateLink = (url) => typeof url === 'string' && ALLOWED_PROTOCOL.test(url) && !/^(javascript|vbscript|data):/i.test(url)
const defaultLinkOpen = md.renderer.rules.link_open || ((tokens, idx, options, env, self) => self.renderToken(tokens, idx, options))
md.renderer.rules.link_open = (tokens, idx, options, env, self) => {
    const href = tokens[idx].attrGet('href') || ''
    if (/^https?:/i.test(href)) {
        tokens[idx].attrSet('target', '_blank')
        tokens[idx].attrSet('rel', 'noopener noreferrer')
    }
    return defaultLinkOpen(tokens, idx, options, env, self)
}
md.renderer.rules.table_open = () => '<div class="md-table-wrap"><table>'  // 表格包一层横向滚动容器，窄窗口下不撑破消息气泡
md.renderer.rules.table_close = () => '</table></div>'
const completeEmphasis = (text) => {
    const lines = text.split('\n')
    const last = lines[lines.length - 1] || ''
    if (/^\s*[*+-]\s/.test(last)) return text  // 行首列表符号不参与配对
    const singles = (last.replace(/\*\*/g, '').match(/\*/g) || []).length
    return singles % 2 === 1 ? `${text}*` : text
}
const completeLink = (text) => {
    const line = (text.split('\n').pop() || '').replace(/`[^`]*`/g, '').replace(/\\[[\]]/g, '')  // 行内代码与转义方括号不参与配对；只补 ]，不编造 ]() 避免生成空链接
    return (line.match(/\[/g) || []).length > (line.match(/\]/g) || []).length ? `${text}]` : text
}
const completeStream = (text) => {
    if ((text.match(/```/g) || []).length % 2 === 1) return `${text}\n\`\`\``
    let out = text
    if ((out.match(/\*\*/g) || []).length % 2 === 1) out += '**'
    if ((out.match(/`/g) || []).length % 2 === 1) out += '`'
    return completeLink(completeEmphasis(out))
}
// 按闭合围栏切分：文本段继续走 markdown-it，代码段交给 CodeEditor；流式未闭合围栏按代码段处理
const segmentsOf = (text) => {
    const lines = String(text || '').split('\n')
    const out = []
    let buffer = []
    let key = 0
    const flush = () => { if (buffer.length) { out.push({ key: key++, type: 'text', text: buffer.join('\n') }); buffer = [] } }
    for (let index = 0; index < lines.length; index += 1) {
        const head = /^\s*```\s*([^\s`]*)/.exec(lines[index])
        if (!head) { buffer.push(lines[index]); continue }
        flush()
        const lang = String(head[1] || '').trim()
        const code = []
        index += 1
        while (index < lines.length && !/^\s*```\s*$/.test(lines[index])) { code.push(lines[index]); index += 1 }
        out.push({ key: key++, type: lang === 'mermaid' ? 'mermaid' : 'code', lang, code: code.join('\n') })
    }
    flush()
    return out
}
const themeTick = ref(0)
const segments = computed(() => {
    void themeTick.value  // 主题切换时重建 mermaid 段，使其按新主题重绘
    const text = props.content || ''
    return segmentsOf(props.streaming ? completeStream(text) : text)
})
const hasStreamingFence = computed(() => Boolean(props.streaming) && (String(props.content || '').match(/```/g) || []).length % 2 === 1)
const renderText = (value) => md.render(value)
const copyCode = async (code) => {
    try {
        await navigator.clipboard.writeText(String(code || ''))
        toast.success('已复制')
    } catch {
        toast.error('复制失败')
    }
}
const rootRef = ref(null)
const mermaidFailed = reactive(new Set())
const svgCache = new Map()  // key = 主题 + 源文本，避免同一图反复绘制
let mermaidInstance = null
let mermaidSeq = 0
const currentTheme = () => (document.documentElement.classList.contains('dark') ? 'dark' : 'default')
const loadMermaid = async () => {
    const theme = currentTheme()
    if (mermaidInstance && mermaidInstanceTheme === theme) return mermaidInstance
    const mod = await import('mermaid')
    const mermaid = mod.default || mod
    mermaid.initialize({ startOnLoad: false, securityLevel: 'strict', theme, fontFamily: 'inherit' })
    mermaidInstance = mermaid
    mermaidInstanceTheme = theme
    return mermaidInstance
}
let mermaidInstanceTheme = ''
const paintMermaid = async () => {
    const root = rootRef.value
    if (!root) return
    const blocks = [...root.querySelectorAll('.md-mermaid')].filter((el) => !el.dataset.painted)
    if (!blocks.length) return
    for (const el of blocks) {
        if (props.streaming) continue  // 流式期间不绘图也不能打标，否则结束后 Vue 不重建 DOM 会让图永久不渲染
        el.dataset.painted = '1'
        const src = el.querySelector('code')?.textContent || ''
        if (!src) continue
        const key = `${currentTheme()}::${src}`
        try {
            let svg = svgCache.get(key)
            if (!svg) {
                const mermaid = await loadMermaid()
                svg = (await mermaid.render(`lwa-mermaid-${++mermaidSeq}`, src)).svg
                svgCache.set(key, svg)
            }
            const pre = el.querySelector('pre')
            if (!pre || !el.isConnected) continue
            const holder = document.createElement('div')
            holder.className = 'md-mermaid-svg'
            holder.innerHTML = svg
            pre.replaceWith(holder)
        } catch {
            mermaidFailed.add(Number(el.dataset.mermaidKey))  // 绘制失败改用 CodeEditor 展示源码
        }
    }
}
let themeObserver = null
onMounted(() => {
    nextTick(paintMermaid)
    themeObserver = new MutationObserver(() => { themeTick.value += 1 })
    themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
})
onBeforeUnmount(() => {
    if (themeObserver) themeObserver.disconnect()
    themeObserver = null
})
watch(segments, () => nextTick(paintMermaid))
watch(() => props.streaming, () => nextTick(paintMermaid))
</script>

<style scoped>
.md-show-wrap { width: 100%; }
.md-show { font-size: 13px; line-height: 1.7; color: var(--foreground); word-break: break-word; }
.md-show :deep(h1) { margin: 16px 0 8px; font-size: 20px; line-height: 1.4; }
.md-show :deep(h2) { margin: 14px 0 8px; font-size: 17px; line-height: 1.4; }
.md-show :deep(h3) { margin: 12px 0 6px; font-size: 15px; line-height: 1.5; }
.md-show :deep(h4), .md-show :deep(h5), .md-show :deep(h6) { margin: 10px 0 6px; font-size: 13px; }
.md-show :deep(p) { margin: 0 0 8px; }
.md-show :deep(ul), .md-show :deep(ol) { padding-left: 20px; margin: 0 0 8px; }
.md-show :deep(li) { margin: 2px 0; }
.md-show :deep(:not(pre) > code) { padding: 1px 5px; border-radius: 4px; background: var(--muted); font-size: 12px; font-family: Consolas, Menlo, 'Courier New', monospace; }
.md-show :deep(.md-table-wrap) { margin: 0 0 10px; overflow-x: auto; }
.md-show :deep(table) { width: 100%; border: 1px solid var(--border); border-collapse: collapse; font-size: 12px; }
.md-show :deep(th) { padding: 4px 8px; border: 1px solid var(--border); background: var(--muted); text-align: left; }
.md-show :deep(td) { padding: 4px 8px; border: 1px solid var(--border); text-align: left; }
.md-show :deep(blockquote) { margin: 0 0 8px; padding: 4px 10px; border-left: 3px solid var(--primary); color: var(--muted-foreground); }
.md-show :deep(a) { color: var(--primary); text-decoration: none; }
.md-show :deep(a:hover) { text-decoration: underline; }
.md-show :deep(hr) { margin: 12px 0; border: none; border-top: 1px solid var(--border); }
.md-code { margin: 0 0 10px; border: 1px solid var(--border); border-radius: 6px; overflow: hidden; }
.md-code-head { display: flex; justify-content: space-between; align-items: center; padding: 4px 10px; background: var(--muted); font-size: 11px; color: var(--muted-foreground); }
.md-code-lang { padding: 1px 6px; border-radius: 4px; background: var(--muted); color: var(--muted-foreground); font-size: 11px; line-height: 1.4; }
.md-code-actions { display: inline-flex; align-items: center; gap: 4px; }
.md-copy-btn { padding: 2px 6px; border: none; border-radius: 4px; background: transparent; color: var(--muted-foreground); font-size: 11px; line-height: 1; cursor: pointer; }
.md-copy-btn { opacity: 0; transition: opacity 0.15s ease; }
.md-code:hover .md-copy-btn, .md-code:focus-within .md-copy-btn { opacity: 1; }
.md-copy-btn:hover { color: var(--primary); background: var(--accent); }
.md-code :deep(.code-editor) { border: none; border-radius: 0; }
.md-mermaid-source { margin: 0; padding: 10px 12px; overflow-x: auto; background: var(--muted); color: var(--muted-foreground); font-size: 12px; line-height: 1.6; font-family: Consolas, Menlo, 'Courier New', monospace; white-space: pre-wrap; word-break: break-word; }
.md-mermaid-svg { padding: 10px 12px; overflow-x: auto; background: var(--muted); }
.md-mermaid-svg svg { display: block; max-width: 100%; height: auto; margin: 0 auto; }
.md-generating { display: inline-flex; align-items: center; gap: 6px; padding: 2px 0; color: var(--muted-foreground); font-size: 12px; animation: md-breathe 1.2s ease-in-out infinite; }
.md-generating-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--primary); }
@keyframes md-breathe { 0%, 100% { opacity: 0.55; } 50% { opacity: 1; } }
</style>
