<template>
    <aside v-show="open" class="right-tools">
        <div class="rt-tabs">
            <button v-for="tab in TABS" :key="tab.key" class="rt-tab" :class="{ active: active === tab.key }" :aria-pressed="active === tab.key" @click="active = tab.key">
                <component :is="tab.icon" :size="14" />
                <span>{{ tab.label }}</span>
            </button>
            <div class="rt-actions">
                <Button variant="ghost" size="icon-sm" class="rt-icon" aria-label="关闭右侧面板" @click="emit('update:open', false)"><X :size="16" /></Button>
            </div>
        </div>
        <div v-if="!active" class="rt-empty">
            <button v-for="item in ENTRIES" :key="item.key" class="rt-entry" @click="active = item.key">
                <component :is="item.icon" :size="16" />
                <span class="rt-entry-label">{{ item.label }}</span>
            </button>
        </div>
        <div v-else class="rt-body">
            <BuildPanel v-show="active === 'build'" ref="buildRef" :tools="builtTools" />
            <FileCode v-show="active === 'file'" ref="fileRef" :project-path="projectPath" />
            <TerminalPanel v-show="active === 'terminal'" ref="terminalRef" />
            <AgencyList v-show="active === 'todo'" :todos="todos" :todo-enabled="todoEnabled" :rules="rules" :skills="skills" :files="contextFiles" :others="contextOthers" :context-tokens="contextTokens" :context-limit="contextLimit" :usage-detail="usageDetail" :compressing="compressing" :disabled-compress="running" @compress="onCompress" />
        </div>
    </aside>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { FolderOpen, List, SquareTerminal, Wrench, X } from 'lucide-vue-next'
import { Button } from '../components/shadcn_ui/ui/button'
import AgencyList from './right_panel/AgencyList.vue'
import BuildPanel from './right_panel/BuildPanel.vue'
import FileCode from './right_panel/FileCode.vue'
import TerminalPanel from './right_panel/TerminalPanel.vue'
const props = defineProps({
    open: { type: Boolean, default: false },
    // 当前页签由父级 Chat.vue 持有：窗口 resize / 收起右栏会重建布局，
    // 若状态留在本组件内就会被重置回空入口页
    activeTab: { type: String, default: '' },
    projectPath: { type: String, default: '' },
    todos: { type: Array, default: () => [] },
    todoEnabled: { type: Boolean, default: true },
    rules: { type: Array, default: () => [] },
    skills: { type: Array, default: () => [] },
    contextFiles: { type: Array, default: () => [] },
    contextOthers: { type: Array, default: () => [] },
    contextTokens: { type: Number, default: 0 },
    contextLimit: { type: Number, default: 0 },
    usageDetail: { type: Object, default: () => ({}) },
    running: { type: Boolean, default: false },
    compressing: { type: Boolean, default: false },
    builtTools: { type: Object, default: () => ({ browser: true, computer: false, word: false, ppt: false, excel: false, mobile: false }) },
})
const emit = defineEmits(['update:open', 'update:activeTab', 'compress'])
const terminalRef = ref(null)  // 面板常驻挂载（v-show 保活）：终端多标签会话与监听器不随切换销毁，便于命令卡联动执行
const fileRef = ref(null)
const buildRef = ref(null)
const TABS = [{ key: 'todo', label: '待办', icon: List }, { key: 'build', label: '内置工具', icon: Wrench }, { key: 'file', label: '文件', icon: FolderOpen }, { key: 'terminal', label: '终端', icon: SquareTerminal }]
const ENTRIES = [{ key: 'todo', label: '待办', icon: List }, { key: 'build', label: '内置工具', icon: Wrench }, { key: 'terminal', label: '终端', icon: SquareTerminal }, { key: 'file', label: '文件', icon: FolderOpen }]
const active = computed({
    get: () => props.activeTab,
    set: (value) => emit('update:activeTab', value),
})
const onTerminalRun = (event) => {  // 命令卡「在终端查看」经此事件切到终端面板并实际执行
    active.value = 'terminal'
    const detail = event.detail || {}
    nextTick(() => { if (terminalRef.value) terminalRef.value.runCommand(detail.command, detail.cwd) })
}
const openPanel = (name) => { active.value = name === 'browser' ? 'build' : name }
const openBuild = (section) => {
    active.value = 'build'
    nextTick(() => { if (buildRef.value && section) buildRef.value.openSection(section) })
}
const openFile = (rel, refresh = false) => {
    if (!rel) return
    active.value = 'file'
    nextTick(() => { if (fileRef.value) fileRef.value.openFile(rel, refresh) })
}
const attach = (sessionId, cwd) => {
    if (!sessionId) return
    active.value = 'terminal'
    nextTick(() => { if (terminalRef.value) terminalRef.value.attachSession(sessionId, cwd) })
}
const onCompress = () => emit('compress')
onMounted(() => window.addEventListener('lwa:terminal-run', onTerminalRun))
onBeforeUnmount(() => window.removeEventListener('lwa:terminal-run', onTerminalRun))
defineExpose({ openPanel, openBuild, openFile, attach })
</script>

<style scoped>
.right-tools { display: flex; flex-direction: column; height: 100%; min-width: 0; min-height: 0; overflow: hidden; background: var(--background); }
.rt-tabs { display: flex; flex: none; align-items: center; gap: 2px; height: 36px; padding: 0 6px; border-bottom: 1px solid var(--border); }
.rt-tab { display: inline-flex; align-items: center; gap: 4px; height: 26px; padding: 0 8px; border: none; border-radius: 6px; background: transparent; color: var(--muted-foreground); font-size: 12px; cursor: pointer; }
.rt-tab:hover { background: var(--muted); }
.rt-tab.active { background: var(--accent); color: var(--foreground); }
.rt-actions { display: flex; align-items: center; gap: 2px; margin-left: auto; }
.rt-icon { width: 26px; height: 26px; padding: 0; }
.rt-empty { flex: 1; display: flex; flex-direction: column; justify-content: center; align-items: center; gap: 4px; min-height: 0; padding: 0 16px; }
.rt-entry { display: flex; align-items: center; gap: 8px; width: min(240px, 100%); height: 34px; padding: 0 10px; border: none; border-radius: 6px; background: transparent; color: var(--foreground); font-size: 13px; cursor: pointer; }
.rt-entry:hover { background: var(--muted); }
.rt-entry-label { flex: 1; text-align: left; }
.rt-body { flex: 1; display: flex; flex-direction: column; min-height: 0; }
</style>
