<template>
    <OfficeOptionEditBase label="Word" title="Word 操作" enable-desc="开启后智能体可使用 12 个 word_* 工具，支持本机任意合法路径并独立自动截图；关闭后拒绝执行并结束后台服务" :fetch-option="getWordOption" :save-option="saveWordOption" />
</template>

<script setup>
import OfficeOptionEditBase from './OfficeOptionEditBase.vue'
import { getWordOption, saveWordOption } from '../../../api/request_data'
</script>
