<template>
    <router-view v-if="kernelReady" />
    <!-- 全局提示与确认框宿主，供 toast / confirm 命令式调用 -->
    <Toaster />
    <ConfirmHost />
    <QuitState />
    <InitState v-if="showInit" @kernel-ready="kernelReady = true" @done="showInit = false" />
</template>

<script setup>
import { ref } from 'vue'
import { Toaster } from './components/shadcn_ui/lib/useToast'
import { ConfirmHost } from './components/shadcn_ui/lib/useConfirm'
import QuitState from './views/others/QuitState.vue'
import InitState from './views/others/InitState.vue'
// 全局深色主题：参考 Trae 的 #181818/#1f1f1f/#252526 层级，主色沿用现有蓝色
const kernelReady = ref(false)
const showInit = ref(true)
</script>

<style>
html, body, #app { height: 100%; margin: 0; }
body { background: var(--background); color: var(--foreground); }
.panel-card { background: var(--card); border: 1px solid var(--border); border-radius: 8px; }
* { box-sizing: border-box; }
</style>
