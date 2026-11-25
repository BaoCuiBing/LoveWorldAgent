<script setup>
import { DropdownMenuLabel } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <DropdownMenuLabel :class="cn('px-2 py-1 text-[11px] font-medium text-muted-foreground', props.class)">
        <slot />
    </DropdownMenuLabel>
</template>
