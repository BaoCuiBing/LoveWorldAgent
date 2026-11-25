<template>
    <div class="bt-root">
        <Tabs v-model="tab">
            <TabsList>
                <TabsTrigger value="browser">浏览器</TabsTrigger>
                <TabsTrigger value="computer">电脑</TabsTrigger>
                <TabsTrigger value="word">Word</TabsTrigger>
                <TabsTrigger value="ppt">PPT</TabsTrigger>
                <TabsTrigger value="excel">Excel</TabsTrigger>
                <TabsTrigger value="mobile">移动端</TabsTrigger>
            </TabsList>
            <TabsContent value="browser" class="bt-content"><BroweringEdit @open-browser="onOpenBrowser" /></TabsContent>
            <TabsContent value="computer" class="bt-content"><ComputerUseEdit /></TabsContent>
            <TabsContent value="word" class="bt-content"><WordOptionEdit /></TabsContent>
            <TabsContent value="ppt" class="bt-content"><PptOptionEdit /></TabsContent>
            <TabsContent value="excel" class="bt-content"><ExcelOptionEdit /></TabsContent>
            <TabsContent value="mobile" class="bt-content"><MobileOptionEdit /></TabsContent>
        </Tabs>
    </div>
</template>

<script setup>
import { ref } from 'vue'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '../../components/shadcn_ui/ui/tabs'
import BroweringEdit from './built_tool_frames/BroweringEdit.vue'
import ComputerUseEdit from './built_tool_frames/ComputerUseEdit.vue'
import WordOptionEdit from './built_tool_frames/WordOptionEdit.vue'
import PptOptionEdit from './built_tool_frames/PptOptionEdit.vue'
import ExcelOptionEdit from './built_tool_frames/ExcelOptionEdit.vue'
import MobileOptionEdit from './built_tool_frames/MobileOptionEdit.vue'
const emit = defineEmits(['open-browser'])
const onOpenBrowser = (payload) => emit('open-browser', payload)
const tab = ref('browser')
</script>

<style scoped>
.bt-root { display: flex; flex-direction: column; gap: 12px; }
.bt-content { margin-top: 14px; }
</style>