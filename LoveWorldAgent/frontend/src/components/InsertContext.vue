<template>
    <div v-if="visible" ref="panelRef" class="context-panel" :class="`panel-${mode}`" @click.stop>
        <template v-if="mode === 'context'">
            <div class="context-list">
                <div v-for="item in categories" :key="item.value" class="context-row" :title="item.path ? `${item.label} ${item.path}` : item.label" @click="pick('context', item)">
                    <component :is="item.icon" class="row-icon" :size="15" />
                    <span class="row-label">{{ item.label }}</span>
                    <span v-if="item.path" class="row-path">{{ item.path }}</span>
                    <ChevronRight class="row-arrow" :size="12" />
                </div>
            </div>
        </template>
        <template v-else-if="mode === 'slash'">
            <div class="group-label">命令</div>
            <div v-for="command in commands" :key="command.value" class="menu-item" @click="pick('command', command)">
                <component :is="command.icon" class="item-icon" :size="15" />
                <div class="item-main">
                    <div class="item-title">{{ command.label }}</div>
                    <div class="item-desc">{{ command.desc }}</div>
                </div>
            </div>
            <div class="group-label">技能</div>
            <div v-if="skillsLoading" class="group-empty">加载中…</div>
            <div v-else-if="skillsError" class="group-empty">{{ skillsError }}</div>
            <div v-else-if="!skills.length" class="group-empty">暂无技能</div>
            <div v-for="skill in skills" :key="skill.name" class="menu-item" @click="pick('skill', skill)">
                <BookOpen class="item-icon" :size="15" />
                <div class="item-main">
                    <div class="item-title">{{ skill.name }}</div>
                    <div class="item-desc">{{ skill.description || '暂无描述' }}</div>
                </div>
            </div>
            <Button variant="outline" size="sm" class="block-btn" @click="onManage">管理</Button>
        </template>
    </div>
</template>

<script setup>
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { BookOpen, ChevronRight, FileText, Folder, Link, Monitor, Ticket } from 'lucide-vue-next'
import { Button } from '@/components/shadcn_ui/ui/button'
import { listSkills } from '../api/request_data'
const props = defineProps({ visible: { type: Boolean, default: false }, mode: { type: String, default: 'context' }, rootPath: { type: String, default: '' } })
const emit = defineEmits(['update:visible', 'select', 'manage'])
const panelRef = ref(null)
const categories = computed(() => [
    { value: 'file', label: 'File', icon: FileText, path: props.rootPath },
    { value: 'folder', label: 'Folder', icon: Folder, path: props.rootPath },
    { value: 'workspace', label: 'Workspace', icon: Monitor, path: '' },
    { value: 'web', label: 'Web', icon: Link, path: '' },
])
const commands = [
    { value: 'spec', label: 'Spec', desc: '根据需求细化完整的规范、任务清单', icon: FileText },
    { value: 'plan', label: 'Plan', desc: '优先规划任务的执行方向，确认后再动手', icon: Ticket },
    { value: 'goal', label: 'Goal', desc: '启动一个以目标为导向的任务，持续推进', icon: Link },
]
const skills = ref([])
const skillsLoading = ref(false)
const skillsError = ref('')
const loadSkills = async () => {
    if (skills.value.length || skillsLoading.value) return
    skillsLoading.value = true
    skillsError.value = ''
    try {
        const data = await listSkills()
        skills.value = data.list || []
    } catch (err) {
        skillsError.value = `读取技能失败：${err.message}`
    } finally {
        skillsLoading.value = false
    }
}
const close = () => emit('update:visible', false)
const pick = (kind, item) => {
    emit('select', { kind, item })
    close()
}
const onManage = () => {
    emit('manage')
    close()
}
const onDocClick = (event) => {
    if (panelRef.value && !panelRef.value.contains(event.target)) close()
}
const onKeydown = (event) => {
    if (event.key === 'Escape') close()
}
const unbind = () => {
    document.removeEventListener('mousedown', onDocClick)
    window.removeEventListener('keydown', onKeydown)
}
watch(() => [props.visible, props.mode], ([visible, mode]) => {
    if (visible && mode === 'slash') loadSkills()
    if (visible) {
        document.addEventListener('mousedown', onDocClick)
        window.addEventListener('keydown', onKeydown)
    } else {
        unbind()
    }
}, { immediate: true })
onBeforeUnmount(unbind)
</script>

<style scoped>
.context-panel { position: absolute; left: 0; bottom: calc(100% + 8px); z-index: 2000; display: flex; flex-direction: column; max-height: 420px; padding: 6px; overflow-y: auto; border: 1px solid var(--border); border-radius: 10px; background: var(--popover); box-shadow: 0 4px 16px rgb(0 0 0 / 0.28); }
.panel-context { width: 240px; }
.panel-slash { width: 320px; }
.panel-slash .menu-item { padding-left: 16px; }
.context-list { display: flex; flex-direction: column; gap: 1px; }
.context-row { display: flex; align-items: center; gap: 8px; padding: 7px 8px; border-radius: 6px; cursor: pointer; }
.context-row:hover { background: var(--accent); }
.row-icon { flex: none; color: var(--foreground); }
.row-label { flex: none; font-size: 13px; color: var(--foreground); }
.row-path { flex: 1; min-width: 0; overflow: hidden; font-size: 11px; text-align: right; text-overflow: ellipsis; white-space: nowrap; color: var(--muted-foreground); }
.row-arrow { flex: none; color: var(--muted-foreground); }
.group-label { padding: 6px 8px 2px; font-size: 12px; color: var(--muted-foreground); }
.group-empty { padding: 6px 8px; font-size: 12px; color: var(--muted-foreground); }
.menu-item { display: flex; align-items: center; gap: 8px; padding: 7px 8px; border-radius: 6px; cursor: pointer; }
.menu-item:hover { background: var(--accent); }
.item-icon { flex: none; color: var(--foreground); }
.item-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
.item-title { font-size: 13px; color: var(--foreground); }
.item-desc { overflow: hidden; font-size: 11px; text-overflow: ellipsis; white-space: nowrap; color: var(--muted-foreground); }
.item-check { flex: none; color: #22c55e; }
.block-btn { width: 100%; margin-top: 6px; }
</style>
