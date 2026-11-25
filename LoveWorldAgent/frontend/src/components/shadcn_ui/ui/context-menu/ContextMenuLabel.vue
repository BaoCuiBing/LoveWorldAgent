<script setup>
import { ContextMenuLabel } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <ContextMenuLabel :class="cn('px-2 py-1 text-[11px] font-medium text-muted-foreground', props.class)">
        <slot />
    </ContextMenuLabel>
</template>
