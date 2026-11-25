<script setup>
import { TooltipProvider, TooltipRoot } from 'reka-ui'
const props = defineProps({
    open: { type: Boolean, default: undefined },
    defaultOpen: Boolean,
    delayDuration: { type: Number, default: undefined },
})
const emits = defineEmits(['update:open'])
</script>

<template>
    <TooltipProvider>
        <TooltipRoot
            :open="props.open"
            :default-open="props.defaultOpen"
            :delay-duration="props.delayDuration"
            @update:open="emits('update:open', $event)"
        >
            <slot />
        </TooltipRoot>
    </TooltipProvider>
</template>
