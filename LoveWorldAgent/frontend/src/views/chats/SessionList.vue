<template>
    <div class="session-list">
        <div v-if="loading" class="session-mask">
            <LoaderCircle class="session-spinner" :size="18" />
        </div>
        <div v-for="row in rows" :key="row.node.id" class="session-row" :data-depth="row.depth">
            <div class="session-card" :class="[row.node.status, { active: row.node.id === activeId }]" @click="onSelect(row.node)">
                <span class="status-icon" :class="{ spin: row.node.status === 'running' }">
                    <LoaderCircle v-if="row.node.status === 'running'" :size="14" />
                    <CircleX v-else-if="row.node.status === 'failed'" :size="14" />
                    <CircleCheck v-else-if="row.node.status === 'completed'" :size="14" />
                    <CircleMinus v-else-if="row.node.status === 'interrupted'" :size="14" />
                    <CircleCheck v-else :size="14" />
                </span>
                <div class="session-main">
                    <div class="session-title">{{ row.node.title }}</div>
                    <div class="session-meta">{{ statusText(row.node.status) }} · {{ formatTime(row.node.updated_at) }}</div>
                </div>
                <div class="session-actions" @click.stop>
                    <DropdownMenu>
                        <DropdownMenuTrigger as-child>
                            <Button variant="ghost" size="icon-sm" aria-label="会话操作">
                                <Ellipsis :size="14" />
                            </Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end" class="w-32">
                            <DropdownMenuItem @select="onCommand('rename', row.node)">重命名</DropdownMenuItem>
                            <DropdownMenuItem @select="onCommand('delete', row.node)">删除</DropdownMenuItem>
                        </DropdownMenuContent>
                    </DropdownMenu>
                </div>
            </div>
        </div>
        <div v-if="!sessions.length" class="session-empty">
            <div class="session-empty-icon"><MessageSquare :size="20" /></div>
            <div class="session-empty-title">暂无会话</div>
            <div class="session-empty-desc">点击上方「新建会话」开始一段新的对话</div>
        </div>
    </div>
</template>

<script setup>
import { computed } from 'vue'
import { CircleCheck, CircleMinus, CircleX, Ellipsis, LoaderCircle, MessageSquare } from 'lucide-vue-next'
import { Button } from '@/components/shadcn_ui/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/shadcn_ui/ui/dropdown-menu'
const props = defineProps({
    sessions: { type: Array, default: () => [] },
    activeId: { type: String, default: '' },
    keyword: { type: String, default: '' },
    loading: { type: Boolean, default: false },
})
const emit = defineEmits(['select', 'rename', 'delete'])
// 自研递归树：按关键字过滤后深度优先展开，祖先命中子节点时保留父级，默认全展开（indent 0）
const rows = computed(() => {
    const keyword = String(props.keyword || '').trim().toLowerCase()
    const matched = (node) => !keyword || String(node.title || '').toLowerCase().includes(keyword)
    const visible = (node) => matched(node) || (Array.isArray(node.children) && node.children.some(visible))
    const list = []
    const walk = (nodes, depth) => {
        nodes.forEach((node) => {
            if (!visible(node)) return
            list.push({ node, depth })
            if (Array.isArray(node.children) && node.children.length) walk(node.children, depth + 1)
        })
    }
    walk(props.sessions, 0)
    return list
})
const onSelect = (data) => emit('select', data)
const onCommand = (command, data) => emit(command, data)
const statusText = (status) => ({ running: '进行中', completed: '任务完成', interrupted: '任务中断', failed: '任务异常' }[status] || '任务完成')
const formatTime = (value) => {
    const date = new Date(String(value || '').replace(' ', 'T'))
    if (Number.isNaN(date.getTime())) return value || ''
    const now = new Date()
    const time = `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`
    if (date.toDateString() === now.toDateString()) return time
    const yesterday = new Date(now)
    yesterday.setDate(now.getDate() - 1)
    if (date.toDateString() === yesterday.toDateString()) return `昨天 ${time}`
    return `${date.getMonth() + 1}月${date.getDate()}日 ${time}`
}
</script>

<style scoped>
.session-list { position: relative; flex: 1; min-height: 0; width: 100%; overflow-y: auto; }
.session-mask { position: absolute; inset: 0; z-index: 5; display: flex; align-items: flex-start; justify-content: center; padding-top: 32px; background: var(--card); opacity: 0.7; }
.session-spinner { color: var(--primary); animation: spin 1s linear infinite; }
.session-row { width: 100%; }
.session-card { position: relative; display: flex; align-items: center; gap: 10px; width: 100%; box-sizing: border-box; margin: 4px 0; padding: 10px 12px; border: 1px solid transparent; border-radius: 8px; background: var(--card); cursor: pointer; }
.session-card:hover { background: var(--accent); }
.session-card.active { background: var(--muted); border-color: var(--border); }
.session-card.active::before { content: ""; position: absolute; left: 0; top: 8px; bottom: 8px; width: 3px; border-radius: 2px; background: var(--primary); }
.session-card.active .session-title { font-weight: 600; }
:global(html:not(.dark)) .session-card { border-color: var(--border); background: #fff; }
:global(html:not(.dark)) .session-card:hover { border-color: transparent; background: #f5f7fa; }
:global(html:not(.dark)) .session-card.active { border-color: var(--border); background: #eef1f6; }
.status-icon { flex: none; display: inline-flex; align-items: center; color: var(--muted-foreground); }
.session-card.running .status-icon { color: var(--primary); }
.session-card.completed .status-icon { color: #22c55e; }
.session-card.interrupted .status-icon { color: var(--muted-foreground); }
.session-card.failed .status-icon { color: var(--destructive); }
.status-icon.spin { animation: spin 1s linear infinite; }
.session-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
.session-title { overflow: hidden; font-size: 13px; text-overflow: ellipsis; white-space: nowrap; }
.session-meta { font-size: 11px; color: var(--muted-foreground); }
.session-actions { flex: none; display: flex; align-items: center; gap: 2px; opacity: 0; transition: opacity 0.15s ease; }
.session-card:hover .session-actions, .session-card.active .session-actions { opacity: 1; }
.session-empty { display: flex; flex-direction: column; align-items: center; gap: 4px; padding: 24px 12px; text-align: center; }
.session-empty-icon { display: inline-flex; align-items: center; justify-content: center; width: 48px; height: 48px; border-radius: 10px; background: var(--muted); color: var(--muted-foreground); }
.session-empty-title { font-size: 12px; font-weight: 600; color: var(--foreground); }
.session-empty-desc { font-size: 11px; line-height: 1.6; color: var(--muted-foreground); }
@keyframes spin { to { transform: rotate(360deg); } }
</style>
