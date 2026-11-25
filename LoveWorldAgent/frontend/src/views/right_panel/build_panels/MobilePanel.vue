<template>
    <SidecarPanelBase
        :icon="Smartphone"
        empty-title="暂无移动端操作"
        empty-desc="智能体调用 mobile_* 设备控制工具后会在此列出步骤"
        :fetch-state="mobileOptionState"
        :fetch-steps="mobileOptionSteps"
        :stop-sidecar="stopMobileOption"
        @meta="emit('meta', $event)"
    />
</template>

<script setup>
import { Smartphone } from 'lucide-vue-next'
import SidecarPanelBase from './SidecarPanelBase.vue'
import { mobileOptionState, mobileOptionSteps, stopMobileOption } from '../../../api/request_data'
const emit = defineEmits(['meta'])
</script>
