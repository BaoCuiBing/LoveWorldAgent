<template>
    <div class="build-panel">
        <section v-for="sec in sections" :key="sec.key" class="build-acc" :class="{ open: activeKey === sec.key }">
            <button type="button" class="build-acc-head" :aria-expanded="activeKey === sec.key" @click="toggleSection(sec.key)">
                <component :is="sec.icon" class="build-acc-icon" :size="14" />
                <span class="build-acc-title">{{ sec.label }}</span>
                <span v-if="!sec.enabled" class="build-acc-off">未启用</span>
                <span v-else-if="meta[sec.key].running" class="build-acc-live">运行中</span>
                <span class="build-acc-count">{{ meta[sec.key].count }}</span>
                <ChevronDown class="build-acc-arrow" :class="{ open: activeKey === sec.key }" :size="13" />
            </button>
            <div v-show="activeKey === sec.key" class="build-acc-body">
                <BroweringPanel v-if="sec.key === 'browser'" :embedded="true" @meta="onMeta('browser', $event)" />
                <ComputerUsePanel v-else-if="sec.key === 'computer'" @meta="onMeta('computer', $event)" />
                <MobilePanel v-else-if="sec.key === 'mobile'" @meta="onMeta('mobile', $event)" />
                <OfficeOptionPanel v-else-if="sec.key === 'office'" :word="tools.word" :ppt="tools.ppt" :excel="tools.excel" @meta="onMeta('office', $event)" />
            </div>
        </section>
    </div>
</template>

<script setup>
import { computed, reactive, ref, watch } from 'vue'
import { ChevronDown, Cpu, FileSpreadsheet, Globe, Smartphone } from 'lucide-vue-next'
import BroweringPanel from './build_panels/BroweringPanel.vue'
import ComputerUsePanel from './build_panels/ComputerUsePanel.vue'
import MobilePanel from './build_panels/MobilePanel.vue'
import OfficeOptionPanel from './build_panels/OfficeOptionPanel.vue'
const props = defineProps({
    tools: { type: Object, default: () => ({ browser: true, computer: false, word: false, ppt: false, excel: false, mobile: false }) },
})
const meta = reactive({
    browser: { count: 0, running: false },
    computer: { count: 0, running: false },
    mobile: { count: 0, running: false },
    office: { count: 0, running: false },
})
const sections = computed(() => [
    { key: 'browser', label: '内置浏览器', icon: Globe, enabled: Boolean(props.tools.browser) },
    { key: 'computer', label: '电脑控制', icon: Cpu, enabled: Boolean(props.tools.computer) },
    { key: 'mobile', label: '移动端', icon: Smartphone, enabled: Boolean(props.tools.mobile) },
    { key: 'office', label: 'Office 办公', icon: FileSpreadsheet, enabled: Boolean(props.tools.word || props.tools.ppt || props.tools.excel) },
])
const activeKey = ref('')
watch(sections, (list) => {
    if (!activeKey.value) {
        const firstEnabled = list.find((item) => item.enabled) || list[0]
        if (firstEnabled) activeKey.value = firstEnabled.key
    }
}, { immediate: true })
const toggleSection = (key) => {
    activeKey.value = activeKey.value === key ? '' : key
}
const openSection = (key) => {
    if (sections.value.some((item) => item.key === key)) activeKey.value = key
}
const onMeta = (key, payload) => {
    if (!payload) return
    meta[key].count = Number(payload.count || 0)
    meta[key].running = Boolean(payload.running)
}
defineExpose({ openSection })
</script>

<style scoped>
.build-panel { display: flex; flex-direction: column; flex: 1; min-height: 0; overflow: hidden; }
.build-acc { display: flex; flex-direction: column; flex: none; border-bottom: 1px solid var(--border); min-height: 0; }
.build-acc.open { flex: 1; }
.build-acc-head { display: flex; align-items: center; gap: 6px; width: 100%; height: 34px; padding: 0 10px; border: none; background: transparent; color: var(--foreground); font-size: 12px; cursor: pointer; text-align: left; }
.build-acc-head:hover { background: var(--muted); }
.build-acc-icon { flex: none; color: var(--primary); }
.build-acc-title { font-weight: 600; }
.build-acc-live { padding: 1px 6px; border-radius: 8px; background: color-mix(in srgb, #67c23a 15%, transparent); color: #67c23a; font-size: 10px; }
.build-acc-off { padding: 1px 6px; border-radius: 8px; background: var(--muted); color: var(--muted-foreground); font-size: 10px; }
.build-acc-count { margin-left: auto; padding: 0 6px; border-radius: 8px; background: var(--accent); color: var(--muted-foreground); font-size: 11px; }
.build-acc-arrow { flex: none; color: var(--muted-foreground); transition: transform 0.15s ease; transform: rotate(-90deg); }
.build-acc-arrow.open { transform: rotate(0deg); }
.build-acc-body { display: flex; flex-direction: column; flex: 1; min-height: 0; padding-left: 8px; border-top: 1px solid var(--border); }
</style>
