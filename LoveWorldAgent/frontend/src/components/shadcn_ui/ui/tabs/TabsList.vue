<script setup>
import { TabsList } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <TabsList :class="cn('inline-flex h-7 items-center justify-center gap-0.5 rounded-md bg-muted p-0.5 text-muted-foreground', props.class)">
        <slot />
    </TabsList>
</template>
