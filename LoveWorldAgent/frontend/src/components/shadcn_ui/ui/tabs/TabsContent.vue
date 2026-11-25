<script setup>
import { TabsContent } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    value: { type: [String, Number], required: true },
    forceMount: Boolean,
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <TabsContent :value="props.value" :force-mount="props.forceMount" :class="cn('outline-none', props.class)">
        <slot />
    </TabsContent>
</template>
