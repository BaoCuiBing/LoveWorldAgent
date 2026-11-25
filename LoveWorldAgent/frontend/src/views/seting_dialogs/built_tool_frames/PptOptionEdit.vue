<template>
    <OfficeOptionEditBase label="PPT" title="PPT 操作" enable-desc="开启后智能体可使用 10 个 ppt_* 工具，支持本机任意合法路径并独立自动截图；关闭后直接拒绝执行" :fetch-option="getPptOption" :save-option="savePptOption" />
</template>

<script setup>
import OfficeOptionEditBase from './OfficeOptionEditBase.vue'
import { getPptOption, savePptOption } from '../../../api/request_data'
</script>
