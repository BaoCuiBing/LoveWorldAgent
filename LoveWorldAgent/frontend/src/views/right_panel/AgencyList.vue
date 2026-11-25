<template>
    <div class="agency-list">
        <section v-if="todoEnabled" class="ag-section ag-todos">
            <div class="ag-head clickable" @click="todoOpen = !todoOpen">
                <ChevronRight class="ag-arrow" :class="{ open: todoOpen }" :size="12" />
                <span class="ag-title">待办</span>
                <span v-if="todos.length" class="ag-count">{{ todos.length }}</span>
            </div>
            <div v-show="todoOpen" class="ag-body">
                <div v-for="(item, index) in todos" :key="index" class="ag-row" :title="item.content">
                    <CircleCheckBig v-if="statusOf(item) === 'completed'" class="ag-state done" :size="13" />
                    <LoaderCircle v-else-if="statusOf(item) === 'in_progress'" class="ag-state running animate-spin" :size="13" />
                    <CircleCheck v-else class="ag-state pending" :size="13" />
                    <span class="ag-text" :class="{ finished: statusOf(item) === 'completed' }">{{ item.content }}</span>
                </div>
                <div v-if="!todos.length" class="ag-empty">
                    <div class="ag-empty-icon"><Ticket :size="20" /></div>
                    <div class="ag-empty-title">暂无待办</div>
                    <div class="ag-empty-desc">复杂任务的进展会显示在这里</div>
                </div>
            </div>
        </section>
        <div v-if="todoEnabled" class="ag-divider" />
        <section class="ag-section ag-context-section">
            <div class="ag-head clickable" @click="contextOpen = !contextOpen">
                <ChevronRight class="ag-arrow" :class="{ open: contextOpen }" :size="12" />
                <span class="ag-title">上下文</span>
                <Tooltip>
                    <TooltipTrigger as-child>
                        <span class="ag-info-wrap" aria-label="上下文占用说明"><Info class="ag-info" :size="12" /></span>
                    </TooltipTrigger>
                    <TooltipContent>当前会话已用上下文占窗口上限的比例；下方标签页为注入到提示词的规则、技能、文件与工具产出</TooltipContent>
                </Tooltip>
                <button class="ag-compress" type="button" :disabled="compressing || disabledCompress" @click.stop="emit('compress')">{{ compressing ? '压缩中…' : '压缩' }}</button>
            </div>
            <div v-show="contextOpen" class="ag-body ag-context-body">
                <div class="ag-context">
                    <Tooltip>
                        <TooltipTrigger as-child>
                            <div class="ag-progress-wrap">
                                <Progress class="ag-progress" :model-value="percent" :max="100" />
                            </div>
                        </TooltipTrigger>
                        <TooltipContent>已用 {{ percentLabel }}%（{{ formatTokens(cumulativeTokens) }} / {{ formatTokens(contextLimit) }}）</TooltipContent>
                    </Tooltip>
                    <HoverCard :open-delay="120">
                        <HoverCardTrigger as-child>
                            <span class="ag-tokens">Tokens: {{ formatTokens(cumulativeTokens) }}</span>
                        </HoverCardTrigger>
                        <HoverCardContent side="top" align="end" class="ag-usage-card w-[230px]">
                            <div class="ag-usage">
                                <div class="ag-usage-title">本会话累计消耗</div>
                                <div v-for="row in usageRows" :key="row.label" class="ag-usage-row">
                                    <span>{{ row.label }}</span>
                                    <span>{{ formatTokens(row.value) }}</span>
                                </div>
                                <div class="ag-usage-row total"><span>合计</span><span>{{ formatTokens(cumulativeTokens) }}</span></div>
                                <div class="ag-usage-row"><span>缓存命中率</span><span>{{ hitRate }}%</span></div>
                                <div class="ag-usage-bar">
                                    <span class="ag-bar-read" :style="{ width: barPercents.read + '%' }" />
                                    <span class="ag-bar-write" :style="{ width: barPercents.write + '%' }" />
                                    <span class="ag-bar-miss" :style="{ width: barPercents.miss + '%' }" />
                                </div>
                                <div class="ag-usage-legend"><span>命中</span><span>写入</span><span>未命中</span></div>
                            </div>
                        </HoverCardContent>
                    </HoverCard>
                </div>
                <Tabs v-model="tabKey" class="ag-tabs">
                    <TabsList class="ag-tab-list">
                        <TabsTrigger v-for="tab in TABS" :key="tab.key" :value="tab.key" class="ag-tab">
                            <span class="ag-tab-label">{{ tab.label }}</span>
                            <span class="ag-badge">{{ countOf(tab.key) }}</span>
                        </TabsTrigger>
                    </TabsList>
                    <TabsContent value="rules" class="ag-tab-content">
                        <div class="ag-pane">
                            <div v-for="item in rules" :key="item.name" class="ag-row" :title="item.preview || item.name">
                                <FileText class="ag-item-icon" :size="13" />
                                <span class="ag-text">{{ item.name }}</span>
                                <span class="ag-tag" :class="{ always: item.trigger === 'always_on' }">{{ triggerText(item.trigger) }}</span>
                            </div>
                            <div v-if="!rules.length" class="ag-empty">
                                <div class="ag-empty-icon"><FileText :size="20" /></div>
                                <div class="ag-empty-title">暂无规则</div>
                                <div class="ag-empty-desc">常驻规则与智能生效规则会显示在这里</div>
                            </div>
                        </div>
                    </TabsContent>
                    <TabsContent value="skills" class="ag-tab-content">
                        <div class="ag-pane">
                            <div v-for="skill in skills" :key="skill.name" class="ag-row" :title="skill.description || skill.name">
                                <BookOpen class="ag-item-icon" :size="13" />
                                <span class="ag-text">{{ skill.name }}</span>
                            </div>
                            <div v-if="!skills.length" class="ag-empty">
                                <div class="ag-empty-icon"><BookOpen :size="20" /></div>
                                <div class="ag-empty-title">暂无已启用技能</div>
                                <div class="ag-empty-desc">在设置-技能中启用后显示</div>
                            </div>
                        </div>
                    </TabsContent>
                    <TabsContent value="files" class="ag-tab-content">
                        <div class="ag-pane">
                            <div v-for="rel in files" :key="rel" class="ag-row" :title="rel">
                                <img class="ag-item-img" :src="fileIconOf(rel)" :alt="rel" />
                                <span class="ag-text">{{ rel }}</span>
                            </div>
                            <div v-if="!files.length" class="ag-empty">
                                <div class="ag-empty-icon"><FolderOpen :size="20" /></div>
                                <div class="ag-empty-title">暂无文件上下文</div>
                                <div class="ag-empty-desc">对话中被引用的文件会显示在这里</div>
                            </div>
                        </div>
                    </TabsContent>
                    <TabsContent value="others" class="ag-tab-content">
                        <div class="ag-pane">
                            <div v-for="item in others" :key="item.label" class="ag-row" :title="item.path || `${item.label}：${item.kind}`">
                                <img v-if="item.path" class="ag-item-img" :src="fileIconOf(item.path)" :alt="item.label" />
                                <FileText v-else class="ag-item-icon" :size="13" />
                                <span class="ag-text">{{ item.label }}</span>
                                <span class="ag-tag">{{ item.kind }}</span>
                            </div>
                            <div v-if="!others.length" class="ag-empty">
                                <div class="ag-empty-icon"><Ellipsis :size="20" /></div>
                                <div class="ag-empty-title">暂无其他上下文</div>
                                <div class="ag-empty-desc">用于系统级指令和后台处理的上下文</div>
                            </div>
                        </div>
                    </TabsContent>
                </Tabs>
            </div>
        </section>
    </div>
</template>

<script setup>
import { computed, ref } from 'vue'
import { BookOpen, ChevronRight, CircleCheck, CircleCheckBig, Ellipsis, FileText, FolderOpen, Info, LoaderCircle, Ticket } from 'lucide-vue-next'
import { HoverCard, HoverCardContent, HoverCardTrigger } from '../../components/shadcn_ui/ui/hover-card'
import { Progress } from '../../components/shadcn_ui/ui/progress'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '../../components/shadcn_ui/ui/tabs'
import { Tooltip, TooltipContent, TooltipTrigger } from '../../components/shadcn_ui/ui/tooltip'
import { fileIconOf } from '../../assets/fileIcons'
const props = defineProps({
    todos: { type: Array, default: () => [] },
    todoEnabled: { type: Boolean, default: true },
    rules: { type: Array, default: () => [] },
    skills: { type: Array, default: () => [] },
    files: { type: Array, default: () => [] },
    others: { type: Array, default: () => [] },
    contextTokens: { type: Number, default: 0 },
    contextLimit: { type: Number, default: 0 },
    usageDetail: { type: Object, default: () => ({}) },
    compressing: { type: Boolean, default: false },
    disabledCompress: { type: Boolean, default: false },
})
const emit = defineEmits(['compress'])
const TABS = [{ key: 'rules', label: '规则' }, { key: 'skills', label: '技能' }, { key: 'files', label: '文件' }, { key: 'others', label: '其他' }]
const TRIGGERS = { always_on: '始终生效', model_decision: '智能生效' }
const tabKey = ref('rules')
const todoOpen = ref(true)
const contextOpen = ref(true)
const statusOf = (item) => {
    const value = String((item && item.status) || 'pending').toLowerCase()
    return value === 'completed' || value === 'in_progress' ? value : 'pending'
}
const triggerText = (trigger) => TRIGGERS[String(trigger || '')] || String(trigger || '')
const detail = computed(() => props.usageDetail || {})
const cumulativeTokens = computed(() => Number(detail.value.total || 0))
const rawPercent = computed(() => {
    const limit = Number(props.contextLimit || 0)
    if (!limit || !cumulativeTokens.value) return 0
    return Math.min(100, Math.max(0, (cumulativeTokens.value / limit) * 100))
})
const percent = computed(() => (rawPercent.value > 0 ? Math.max(1, rawPercent.value) : 0))
const percentLabel = computed(() => (rawPercent.value >= 10 || rawPercent.value === 0 ? Math.round(rawPercent.value) : rawPercent.value.toFixed(1)))
const formatTokens = (value) => Number(value || 0).toLocaleString('en-US')
const usageRows = computed(() => {
    const input = Number(detail.value.input || 0)
    const output = Number(detail.value.output || 0)
    const cached = Number(detail.value.cache_read || 0)
    const written = Number(detail.value.cache_write || 0)
    const reasoning = Number(detail.value.reasoning || 0)
    return [{ label: '输入', value: input }, { label: '输出', value: output }, { label: '缓存命中', value: cached }, { label: '缓存未命中', value: Math.max(0, input - cached - written) }, { label: '缓存写入', value: written }, { label: '思考', value: reasoning }, { label: '回复', value: Math.max(0, output - reasoning) }]
})
const hitRate = computed(() => {
    const input = Number(detail.value.input || 0)
    return input ? Math.min(100, Math.round((Number(detail.value.cache_read || 0) / input) * 100)) : 0
})
const barPercents = computed(() => {
    const input = Number(detail.value.input || 0)
    if (!input) return { read: 0, write: 0, miss: 100 }
    const read = Math.min(100, (Number(detail.value.cache_read || 0) / input) * 100)
    const write = Math.min(100 - read, (Number(detail.value.cache_write || 0) / input) * 100)
    return { read, write, miss: Math.max(0, 100 - read - write) }
})
const countOf = (key) => ({ rules: props.rules.length, skills: props.skills.length, files: props.files.length, others: props.others.length }[key] || 0)
</script>

<style scoped>
.agency-list { display: flex; flex-direction: column; height: 100%; min-height: 0; padding: 8px 10px; overflow: hidden; font-size: 12px; }
.ag-section { display: flex; flex-direction: column; gap: 6px; min-height: 0; }
.ag-todos { flex: 35 1 0; }
.ag-context-section { flex: 65 1 0; }
.ag-head { display: flex; align-items: center; gap: 4px; height: 24px; color: var(--muted-foreground); }
.ag-head.clickable { cursor: pointer; }
.ag-info-wrap { display: inline-flex; align-items: center; }
.ag-info { flex: none; color: var(--muted-foreground); }
.ag-arrow { flex: none; color: var(--muted-foreground); transition: transform 0.15s ease; }
.ag-arrow.open { transform: rotate(90deg); }
.ag-title { font-size: 12px; font-weight: 600; color: var(--foreground); }
.ag-count { font-size: 11px; color: var(--muted-foreground); }
.ag-body { display: flex; flex-direction: column; flex: 1; min-height: 0; gap: 1px; overflow: auto; }
.ag-context-body { gap: 8px; overflow: hidden; }
.ag-pane { display: flex; flex-direction: column; flex: 1; min-height: 0; gap: 1px; overflow: auto; }
.ag-row { display: flex; align-items: center; gap: 6px; height: 26px; padding: 0 4px; border-radius: 4px; }
.ag-row:hover { background: var(--accent); }
.ag-state { flex: none; }
.ag-state.done { color: #67c23a; }
.ag-state.running { color: var(--primary); }
.ag-state.pending { color: var(--muted-foreground); }
.ag-item-icon { flex: none; color: var(--muted-foreground); }
.ag-item-img { flex: none; display: block; width: 13px; height: 13px; }
.ag-text { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--foreground); }
.ag-text.finished { color: var(--muted-foreground); }
.ag-other-label { flex: none; color: var(--muted-foreground); }
.ag-other-value { text-align: right; }
.ag-tag { flex: none; padding: 0 5px; border-radius: 8px; background: var(--muted); color: var(--muted-foreground); font-size: 10px; line-height: 15px; }
.ag-tag.always { background: color-mix(in srgb, var(--primary) 12%, transparent); color: var(--primary); }
.ag-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; flex: 1; min-height: 0; gap: 4px; padding: 18px 8px; text-align: center; }
.ag-empty-icon { display: inline-flex; align-items: center; justify-content: center; width: 48px; height: 48px; border-radius: 10px; background: var(--accent); color: var(--muted-foreground); }
.ag-empty-title { font-size: 12px; font-weight: 600; color: var(--foreground); }
.ag-empty-desc { font-size: 11px; line-height: 1.6; color: var(--muted-foreground); }
.ag-divider { flex: none; height: 1px; margin: 8px 0; background: var(--border); }
.ag-context { display: flex; align-items: center; gap: 8px; padding: 0 4px; }
.ag-progress-wrap { flex: 1; min-width: 0; display: flex; align-items: center; }
.ag-progress { width: 100%; height: 5px; }
.ag-compress { flex: none; margin-left: auto; height: 20px; padding: 0 6px; border: 1px solid var(--border); border-radius: 4px; background: transparent; color: var(--muted-foreground); font-size: 11px; cursor: pointer; }
.ag-compress:hover:not(:disabled) { color: var(--primary); border-color: var(--primary); }
.ag-compress:disabled { opacity: 0.5; cursor: not-allowed; }
.ag-tabs { display: flex; flex-direction: column; flex: 1; min-height: 0; }
.ag-tab-list { flex: none; justify-content: flex-start; gap: 0; height: 27px; padding: 0; border-bottom: 1px solid var(--border); border-radius: 0; background: transparent; }
.ag-tab { position: relative; height: 26px; padding: 0 8px; border-radius: 0; color: var(--muted-foreground); }
.ag-tab:hover { color: var(--foreground); background: transparent; }
.ag-tab[data-state='active'] { color: var(--foreground); background: transparent; box-shadow: none; }
.ag-tab[data-state='active']::after { content: ''; position: absolute; left: 8px; right: 8px; bottom: -1px; height: 2px; background: var(--primary); }
.ag-tab-content { flex: 1; min-height: 0; display: flex; flex-direction: column; padding-top: 6px; }
.ag-tab-label { white-space: nowrap; }
.ag-badge { margin-left: 4px; min-width: 16px; padding: 0 4px; border-radius: 8px; background: var(--muted); color: var(--muted-foreground); font-size: 10px; line-height: 14px; text-align: center; }
.ag-tab[data-state='active'] .ag-badge { background: color-mix(in srgb, var(--primary) 12%, transparent); color: var(--primary); }
.ag-tokens { flex: none; color: var(--muted-foreground); font-size: 11px; font-variant-numeric: tabular-nums; white-space: nowrap; border-bottom: 1px dashed var(--border); cursor: pointer; }
.ag-tokens:hover { color: var(--primary); }
.ag-usage-card { box-sizing: border-box; max-width: calc(100vw - 16px); }
.ag-usage { display: flex; flex-direction: column; gap: 4px; min-width: 0; font-size: 12px; }
.ag-usage-title { margin-bottom: 2px; color: var(--foreground); font-weight: 600; }
.ag-usage-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-width: 0; color: var(--muted-foreground); }
.ag-usage-row span { min-width: 0; }
.ag-usage-row span:first-child { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.ag-usage-row span:last-child { flex: none; color: var(--foreground); font-variant-numeric: tabular-nums; }
.ag-usage-row.total { padding-top: 4px; border-top: 1px solid var(--border); font-weight: 600; }
.ag-usage-bar { display: flex; height: 6px; margin-top: 4px; border-radius: 3px; overflow: hidden; background: var(--muted); }
.ag-bar-read { background: #67c23a; }
.ag-bar-write { background: #e6a23c; }
.ag-bar-miss { background: var(--border); }
.ag-usage-legend { display: flex; justify-content: space-between; color: var(--muted-foreground); font-size: 10px; }
</style>
