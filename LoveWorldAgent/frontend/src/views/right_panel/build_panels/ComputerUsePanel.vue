<template>
    <div class="cu-wrap">
        <SidecarPanelBase
            :icon="Cpu"
            empty-title="暂无电脑操作"
            empty-desc="智能体调用 computer_* 桌面自动化工具后会在此列出步骤"
            :fetch-state="computerUseState"
            :fetch-steps="computerUseSteps"
            :stop-sidecar="stopComputerUse"
            :fetch-screenshot="computerUseScreenshot"
            shot-alt="电脑截图"
            @meta="emit('meta', $event)"
        />
    </div>
</template>

<script setup>
import { Cpu } from 'lucide-vue-next'
import SidecarPanelBase from './SidecarPanelBase.vue'
import { computerUseScreenshot, computerUseState, computerUseSteps, stopComputerUse } from '../../../api/request_data'
const emit = defineEmits(['meta'])
</script>

<style scoped>
.cu-wrap { display: flex; flex-direction: column; flex: 1; min-height: 0; padding-left: 4px; }
</style>
