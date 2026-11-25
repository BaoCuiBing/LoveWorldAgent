<script setup>
import { SelectLabel } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <SelectLabel :class="cn('px-2 py-1 text-[11px] font-medium text-muted-foreground', props.class)">
        <slot />
    </SelectLabel>
</template>
