<template>
    <SidecarPanelBase
        :icon="FileSpreadsheet"
        empty-title="暂无 Office 操作"
        empty-desc="智能体调用 word_* / ppt_* / excel_* 工具后会在此列出步骤"
        :fetch-state="officeOptionState"
        :fetch-steps="officeOptionSteps"
        :stop-sidecar="stopOfficeOption"
        :fetch-screenshot="officeOptionScreenshot"
        shot-alt="Office 截图"
        @meta="emit('meta', $event)"
    >
        <template #badges>
            <span v-if="word" class="op-tag">Word</span>
            <span v-if="ppt" class="op-tag">PPT</span>
            <span v-if="excel" class="op-tag">Excel</span>
        </template>
    </SidecarPanelBase>
</template>

<script setup>
import { FileSpreadsheet } from 'lucide-vue-next'
import SidecarPanelBase from './SidecarPanelBase.vue'
import { officeOptionScreenshot, officeOptionState, officeOptionSteps, stopOfficeOption } from '../../../api/request_data'
defineProps({
    word: { type: Boolean, default: false },
    ppt: { type: Boolean, default: false },
    excel: { type: Boolean, default: false },
})
const emit = defineEmits(['meta'])
</script>

<style scoped>
.op-tag { padding: 1px 6px; border-radius: 6px; background: var(--accent); color: var(--muted-foreground); font-size: 10px; }
</style>
