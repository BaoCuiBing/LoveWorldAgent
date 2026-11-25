<template>
    <div ref="host" class="code-editor" :style="{ height: editorHeight }"></div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import * as monaco from 'monaco-editor/editor/editor.api.js'
import 'monaco-editor/languages/definitions/abap/register.js'
import 'monaco-editor/languages/definitions/apex/register.js'
import 'monaco-editor/languages/definitions/azcli/register.js'
import 'monaco-editor/languages/definitions/bat/register.js'
import 'monaco-editor/languages/definitions/bicep/register.js'
import 'monaco-editor/languages/definitions/cameligo/register.js'
import 'monaco-editor/languages/definitions/clojure/register.js'
import 'monaco-editor/languages/definitions/coffee/register.js'
import 'monaco-editor/languages/definitions/cpp/register.js'
import 'monaco-editor/languages/definitions/csharp/register.js'
import 'monaco-editor/languages/definitions/csp/register.js'
import 'monaco-editor/languages/definitions/css/register.js'
import 'monaco-editor/languages/definitions/cypher/register.js'
import 'monaco-editor/languages/definitions/dart/register.js'
import 'monaco-editor/languages/definitions/dockerfile/register.js'
import 'monaco-editor/languages/definitions/ecl/register.js'
import 'monaco-editor/languages/definitions/elixir/register.js'
import 'monaco-editor/languages/definitions/flow9/register.js'
import 'monaco-editor/languages/definitions/freemarker2/register.js'
import 'monaco-editor/languages/definitions/fsharp/register.js'
import 'monaco-editor/languages/definitions/go/register.js'
import 'monaco-editor/languages/definitions/graphql/register.js'
import 'monaco-editor/languages/definitions/handlebars/register.js'
import 'monaco-editor/languages/definitions/hcl/register.js'
import 'monaco-editor/languages/definitions/html/register.js'
import 'monaco-editor/languages/definitions/ini/register.js'
import 'monaco-editor/languages/definitions/java/register.js'
import 'monaco-editor/languages/definitions/javascript/register.js'
import 'monaco-editor/languages/definitions/julia/register.js'
import 'monaco-editor/languages/definitions/kotlin/register.js'
import 'monaco-editor/languages/definitions/less/register.js'
import 'monaco-editor/languages/definitions/lexon/register.js'
import 'monaco-editor/languages/definitions/liquid/register.js'
import 'monaco-editor/languages/definitions/lua/register.js'
import 'monaco-editor/languages/definitions/m3/register.js'
import 'monaco-editor/languages/definitions/markdown/register.js'
import 'monaco-editor/languages/definitions/mdx/register.js'
import 'monaco-editor/languages/definitions/mips/register.js'
import 'monaco-editor/languages/definitions/msdax/register.js'
import 'monaco-editor/languages/definitions/mysql/register.js'
import 'monaco-editor/languages/definitions/objective-c/register.js'
import 'monaco-editor/languages/definitions/pascal/register.js'
import 'monaco-editor/languages/definitions/pascaligo/register.js'
import 'monaco-editor/languages/definitions/perl/register.js'
import 'monaco-editor/languages/definitions/pgsql/register.js'
import 'monaco-editor/languages/definitions/php/register.js'
import 'monaco-editor/languages/definitions/pla/register.js'
import 'monaco-editor/languages/definitions/postiats/register.js'
import 'monaco-editor/languages/definitions/powerquery/register.js'
import 'monaco-editor/languages/definitions/powershell/register.js'
import 'monaco-editor/languages/definitions/protobuf/register.js'
import 'monaco-editor/languages/definitions/pug/register.js'
import 'monaco-editor/languages/definitions/python/register.js'
import 'monaco-editor/languages/definitions/qsharp/register.js'
import 'monaco-editor/languages/definitions/r/register.js'
import 'monaco-editor/languages/definitions/razor/register.js'
import 'monaco-editor/languages/definitions/redis/register.js'
import 'monaco-editor/languages/definitions/redshift/register.js'
import 'monaco-editor/languages/definitions/restructuredtext/register.js'
import 'monaco-editor/languages/definitions/ruby/register.js'
import 'monaco-editor/languages/definitions/rust/register.js'
import 'monaco-editor/languages/definitions/sb/register.js'
import 'monaco-editor/languages/definitions/scala/register.js'
import 'monaco-editor/languages/definitions/scheme/register.js'
import 'monaco-editor/languages/definitions/scss/register.js'
import 'monaco-editor/languages/definitions/shell/register.js'
import 'monaco-editor/languages/definitions/solidity/register.js'
import 'monaco-editor/languages/definitions/sophia/register.js'
import 'monaco-editor/languages/definitions/sparql/register.js'
import 'monaco-editor/languages/definitions/sql/register.js'
import 'monaco-editor/languages/definitions/st/register.js'
import 'monaco-editor/languages/definitions/swift/register.js'
import 'monaco-editor/languages/definitions/systemverilog/register.js'
import 'monaco-editor/languages/definitions/tcl/register.js'
import 'monaco-editor/languages/definitions/twig/register.js'
import 'monaco-editor/languages/definitions/typescript/register.js'
import 'monaco-editor/languages/definitions/typespec/register.js'
import 'monaco-editor/languages/definitions/vb/register.js'
import 'monaco-editor/languages/definitions/wgsl/register.js'
import 'monaco-editor/languages/definitions/xml/register.js'
import 'monaco-editor/languages/definitions/yaml/register.js'
import 'monaco-editor/languages/features/json/register.js'
import 'monaco-editor/languages/features/css/register.js'
import 'monaco-editor/languages/features/html/register.js'
import 'monaco-editor/languages/features/typescript/register.js'
import { normalizeCodeLang } from '../../utils/codeLang.js'
import EditorWorker from 'monaco-editor/editor/editor.worker.js?worker'
import JsonWorker from 'monaco-editor/languages/features/json/json.worker.js?worker'
import CssWorker from 'monaco-editor/languages/features/css/css.worker.js?worker'
import HtmlWorker from 'monaco-editor/languages/features/html/html.worker.js?worker'
import TsWorker from 'monaco-editor/languages/features/typescript/ts.worker.js?worker'
self.MonacoEnvironment = {
    getWorker(workerId, label) {
        if (label === 'json') return new JsonWorker()
        if (['css', 'scss', 'less'].includes(label)) return new CssWorker()
        if (['html', 'handlebars', 'razor'].includes(label)) return new HtmlWorker()
        if (['typescript', 'javascript'].includes(label)) return new TsWorker()
        return new EditorWorker()
    },
}
const props = defineProps({
    modelValue: { type: String, default: '' },
    language: { type: String, default: 'markdown' },
    readOnly: { type: Boolean, default: false },
    height: { type: String, default: '300px' },
    lines: { type: Number, default: 0 },
    maxHeight: { type: Number, default: 360 },
})
const editorHeight = computed(() => (Number(props.lines) > 0 ? `${Math.min(props.maxHeight, Math.max(80, Number(props.lines) * 20 + 24))}px` : props.height))
const emit = defineEmits(['update:modelValue'])
const host = ref(null)
let editor = null
let model = null
let syncing = false
monaco.editor.defineTheme('trae-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: [
        { token: 'keyword', foreground: '7ecbff' },
        { token: 'strong', foreground: 'f0f0f0', fontStyle: 'bold' },
        { token: 'string.link', foreground: '409eff' },
        { token: 'comment', foreground: '7a7a7a' },
    ],
    colors: {
        'editor.background': '#1f1f1f',
        'editor.foreground': '#d0d0d0',
        'editor.lineHighlightBackground': '#2a2a2a',
        'editorLineNumber.foreground': '#5c5c5c',
        'editorLineNumber.activeForeground': '#c6c6c6',
        'editorCursor.foreground': '#409eff',
        'editor.selectionBackground': '#264f78',
    },
})
const themeOf = () => (document.documentElement.classList.contains('dark') ? 'trae-dark' : 'vs')
let themeObserver = null
onMounted(() => {
    model = monaco.editor.createModel(props.modelValue || '', normalizeCodeLang(props.language), monaco.Uri.parse(`inmemory://rules/${Date.now()}-${Math.random().toString(36).slice(2)}`))
    editor = monaco.editor.create(host.value, {
        model,
        theme: themeOf(),
        readOnly: props.readOnly,
        automaticLayout: true,
        minimap: { enabled: false },
        wordWrap: 'on',
        scrollBeyondLastLine: false,
        lineNumbers: 'on',
        fontSize: 13,
        tabSize: 4,
        fontFamily: '"Cascadia Code", Consolas, "Microsoft YaHei", monospace',
        padding: { top: 10, bottom: 10 },
        renderLineHighlight: 'none',
        scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8 },
    })
    editor.onDidChangeModelContent(() => {
        if (!syncing) emit('update:modelValue', editor.getValue())
    })
    themeObserver = new MutationObserver(() => monaco.editor.setTheme(themeOf()))
    themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
})
watch(() => props.modelValue, (value) => {
    if (!model || value === model.getValue()) return
    syncing = true
    model.setValue(value || '')
    syncing = false
})
watch(() => props.language, (value) => {
    if (!model) return
    const next = normalizeCodeLang(value)
    if (model.getLanguageId() !== next) monaco.editor.setModelLanguage(model, next)
})
watch(() => props.readOnly, (value) => editor && editor.updateOptions({ readOnly: value }))
const focus = () => { if (editor) editor.focus() }  // 供右栏打开文件后激活编辑器
defineExpose({ focus })
onBeforeUnmount(() => {
    if (themeObserver) themeObserver.disconnect()
    themeObserver = null
    if (editor) editor.dispose()
    if (model) model.dispose()
    editor = null
    model = null
})
</script>

<style scoped>
.code-editor { width: 100%; overflow: hidden; border: 1px solid var(--border); }
</style>
