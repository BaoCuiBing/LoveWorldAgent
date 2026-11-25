<script setup>
import { TabsTrigger } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    value: { type: [String, Number], required: true },
    disabled: Boolean,
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <TabsTrigger
        :value="props.value"
        :disabled="props.disabled"
        :class="cn('inline-flex h-full items-center justify-center gap-1 whitespace-nowrap rounded px-2.5 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow-sm', props.class)"
    >
        <slot />
    </TabsTrigger>
</template>
