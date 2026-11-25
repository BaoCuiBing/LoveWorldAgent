<script setup>
import { Label } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    for: { type: String, default: undefined },
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <Label
        :for="props.for"
        :class="cn('inline-flex items-center gap-1 text-xs font-medium leading-none text-foreground peer-disabled:cursor-not-allowed peer-disabled:opacity-70', props.class)"
    >
        <slot />
    </Label>
</template>
