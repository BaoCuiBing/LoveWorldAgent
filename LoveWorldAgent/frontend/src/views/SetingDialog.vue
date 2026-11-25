<template>
    <Dialog v-model:open="show">
        <DialogContent class="seting-dialog w-[760px] max-w-[calc(100vw-32px)]" @interact-outside="onInteractOutside">
            <DialogHeader class="seting-head">
                <DialogTitle class="seting-title">设置</DialogTitle>
                <DialogDescription class="sr-only">应用设置：规则、技能、对话、模型、系统、权限、智能体、内置工具与 MCP</DialogDescription>
            </DialogHeader>
            <div class="seting-body">
                <nav class="seting-nav">
                    <div v-for="item in navItems" :key="item.key" class="seting-nav-item" :class="{ on: active === item.key }" @click="active = item.key">
                        <component :is="item.icon" :size="16" />
                        <span>{{ item.label }}</span>
                    </div>
                </nav>
                <div class="seting-content">
                    <RelusEdit v-if="active === 'relus'" />
                    <SkillEdit v-else-if="active === 'skills'" />
                    <ChatSseEdit v-else-if="active === 'chat'" />
                    <ModelEdit v-else-if="active === 'model'" :auto-open="addModelIntent" @opened="addModelIntent = false" />
                    <SystemEdit v-else-if="active === 'system'" />
                    <PermissionEdit v-else-if="active === 'permission'" :intent-mode="permissionIntent" @consumed="permissionIntent = ''" @saved="$emit('permission-saved')" />
                    <AgentEdit v-else-if="active === 'agents'" :auto-open="agentsIntent" @opened="agentsIntent = false" />
                    <BuiltToolsEdit v-else-if="active === 'builtTools'" @open-browser="onOpenBrowser" />
                    <KeepAlive>
                        <McpConfigEdit v-if="active === 'mcp'" />
                    </KeepAlive>
                </div>
            </div>
        </DialogContent>
    </Dialog>
</template>

<script setup>
import { computed, ref, watch } from 'vue'
import { BookOpen, Cable, FileText, Lock, MessageSquare, Settings, User, WandSparkles, Wrench } from 'lucide-vue-next'
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '../components/shadcn_ui/ui/dialog'
import RelusEdit from './seting_dialogs/RelusEdit.vue'
import SkillEdit from './seting_dialogs/SkillEdit.vue'
import ChatSseEdit from './seting_dialogs/ChatSseEdit.vue'
import ModelEdit from './seting_dialogs/ModelEdit.vue'
import SystemEdit from './seting_dialogs/SystemEdit.vue'
import PermissionEdit from './seting_dialogs/PermissionEdit.vue'
import AgentEdit from './seting_dialogs/AgentEdit.vue'
import BuiltToolsEdit from './seting_dialogs/BuiltToolsEdit.vue'
import McpConfigEdit from './seting_dialogs/McpConfigEdit.vue'
const props = defineProps({
    visible: { type: Boolean, default: false },
    openState: { type: Object, default: () => ({}) },
})
const navItems = [{ key: 'relus', label: '规则', icon: FileText }, { key: 'skills', label: '技能', icon: BookOpen }, { key: 'chat', label: '对话', icon: MessageSquare }, { key: 'model', label: '模型', icon: WandSparkles }, { key: 'system', label: '系统', icon: Settings }, { key: 'permission', label: '权限', icon: Lock }, { key: 'agents', label: '智能体', icon: User }, { key: 'builtTools', label: '内置工具', icon: Wrench }, { key: 'mcp', label: 'MCP', icon: Cable }]
const emit = defineEmits(['update:visible', 'permission-saved', 'open-browser'])
const onOpenBrowser = (payload) => emit('open-browser', payload)
// 与原弹窗的「点击遮罩不关闭」保持一致：阻挡外部交互关闭
const onInteractOutside = (event) => event.preventDefault()
const active = ref('relus')
const addModelIntent = ref(false)
const permissionIntent = ref('')
const agentsIntent = ref(false)
watch(() => props.openState, (state) => {
    if (state && state.tab) {
        active.value = state.tab
        if (state.tab === 'model' && state.addModel) addModelIntent.value = true
        if (state.tab === 'permission' && state.permissionMode) permissionIntent.value = state.permissionMode
        if (state.tab === 'agents') agentsIntent.value = Boolean(state.createAgent)
    }
}, { deep: true })
const show = computed({
    get: () => props.visible,
    set: (value) => emit('update:visible', value),
})
</script>

<style scoped>
.seting-dialog { gap: 10px; overflow: hidden; }
.seting-head { text-align: left; }
.seting-title { font-size: 15px; font-weight: 600; }
.seting-body { display: flex; gap: 16px; height: 560px; max-height: calc(100vh - 220px); min-height: 380px; width: 100%; max-width: 100%; min-width: 0; overflow: hidden; }
.seting-nav { display: flex; flex: none; flex-direction: column; gap: 4px; width: 120px; }
.seting-nav-item { display: flex; align-items: center; gap: 8px; padding: 8px 10px; border-radius: 6px; cursor: pointer; color: var(--foreground); font-size: 13px; }
.seting-nav-item.on { background: var(--accent); color: var(--foreground); font-weight: 600; }
.seting-content { flex: 1 1 0; min-width: 0; overflow-y: auto; overflow-x: hidden; }
</style>
